//! Records as JSON text: the pretty form the inspector and editor show, the
//! parser the editor reads back, and the tokens that color it.
//!
//! JSON is a display and edit format here. Values stay `nodedb_types::Value`.
//! Field order comes from the schema, then the remaining keys alphabetically,
//! so one record always prints the same way.

use nodedb_types::{NdbDateTime, Value};

use crate::models::schema::{FieldType, SchemaField};

/// The editor text is not JSON, or not an object.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct JsonError {
    pub message: String,
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn float(f: f64) -> String {
    if !f.is_finite() {
        "null".to_string()
    } else if f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{f:.1}")
    } else {
        f.to_string()
    }
}

/// `2026-06-13T12:41:02Z`, with microseconds only when present.
pub fn iso(at: &NdbDateTime) -> String {
    let c = at.components();
    let base = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        c.year, c.month, c.day, c.hour, c.minute, c.second
    );
    if c.microsecond == 0 {
        format!("{base}Z")
    } else {
        format!("{base}.{:06}Z", c.microsecond)
    }
}

/// A variant with no plain JSON scalar form, through its own serialization.
fn expanded(value: &Value) -> Value {
    sonic_rs::to_string(value)
        .ok()
        .and_then(|text| sonic_rs::from_str::<Value>(&text).ok())
        .map(from_json)
        .unwrap_or(Value::Null)
}

/// A value parsed from JSON text, as that text means it. `Value` reads a
/// JSON array of numbers 0 to 255 as `Bytes`, but JSON has no bytes: such
/// an array is an array of integers.
pub fn from_json(value: Value) -> Value {
    match value {
        Value::Bytes(b) => Value::Array(b.into_iter().map(|n| Value::Integer(n.into())).collect()),
        Value::Array(items) => Value::Array(items.into_iter().map(from_json).collect()),
        Value::Object(map) => {
            Value::Object(map.into_iter().map(|(k, v)| (k, from_json(v))).collect())
        }
        other => other,
    }
}

/// `text` as a decimal the editor can show bare: plain decimal digits that
/// read back from the JSON number (an `f64`) as the same text at `scale`.
/// Any other decimal shows quoted, so it reads back exact.
fn bare_decimal(text: &str, scale: Option<u8>) -> bool {
    let Some(exact) = decimal_text(text, scale) else {
        return false;
    };
    text.trim()
        .parse::<f64>()
        .ok()
        .and_then(|n| decimal_text(&n.to_string(), scale))
        .is_some_and(|back| back == exact)
}

/// A decimal as the editor shows it: bare when it reads back exact, else
/// quoted.
fn decimal_out(text: &str, scale: Option<u8>) -> String {
    if bare_decimal(text, scale) {
        text.trim().to_string()
    } else {
        escape(text)
    }
}

fn field_type<'a>(fields: &'a [SchemaField], name: &str) -> Option<&'a FieldType> {
    fields.iter().find(|f| f.name == name).map(|f| &f.ty)
}

fn ordered_keys<'a>(
    map: &'a std::collections::HashMap<String, Value>,
    fields: &[SchemaField],
) -> Vec<&'a String> {
    let mut keys: Vec<&String> = Vec::with_capacity(map.len());
    for f in fields {
        if let Some((k, _)) = map.get_key_value(&f.name) {
            keys.push(k);
        }
    }
    let mut rest: Vec<&String> = map
        .keys()
        .filter(|k| !fields.iter().any(|f| &f.name == *k))
        .collect();
    rest.sort();
    keys.extend(rest);
    keys
}

/// Writes `value`. `pretty` spreads containers over lines. A vector prints
/// as its first components: the inspector shows it, the editor never edits it.
fn write(
    value: &Value,
    fields: &[SchemaField],
    ty: Option<&FieldType>,
    depth: usize,
    pretty: bool,
    out: &mut String,
) {
    let pad = |n: usize| "  ".repeat(n);
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Integer(i) => out.push_str(&i.to_string()),
        Value::Float(f) => out.push_str(&float(*f)),
        Value::String(s) => match ty {
            Some(FieldType::Decimal { scale, .. }) => out.push_str(&decimal_out(s, *scale)),
            _ => out.push_str(&escape(s)),
        },
        Value::Uuid(s) | Value::Ulid(s) | Value::Regex(s) => out.push_str(&escape(s)),
        Value::DateTime(t) | Value::NaiveDateTime(t) => out.push_str(&escape(&iso(t))),
        Value::Decimal(d) => {
            let scale = if let Some(FieldType::Decimal { scale, .. }) = ty {
                *scale
            } else {
                None
            };
            out.push_str(&decimal_out(&d.to_string(), scale));
        }
        Value::Bytes(b) => out.push_str(&escape(&format!("<{} bytes>", b.len()))),
        Value::Vector(v) => {
            let head: Vec<String> = v.iter().take(3).map(|f| format!("{f}")).collect();
            out.push_str(&format!("[{}, … {} values]", head.join(", "), v.len()));
        }
        Value::Array(items) | Value::Set(items) => {
            if items.is_empty() {
                out.push_str("[]");
            } else if pretty {
                out.push_str("[\n");
                for (i, item) in items.iter().enumerate() {
                    out.push_str(&pad(depth + 1));
                    write(item, fields, None, depth + 1, pretty, out);
                    out.push_str(if i + 1 < items.len() { ",\n" } else { "\n" });
                }
                out.push_str(&pad(depth));
                out.push(']');
            } else {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    write(item, fields, None, depth + 1, pretty, out);
                }
                out.push(']');
            }
        }
        Value::Object(map) => {
            let own: &[SchemaField] = if depth == 0 { fields } else { &[] };
            let keys = ordered_keys(map, own);
            if keys.is_empty() {
                out.push_str("{}");
            } else if pretty {
                out.push_str("{\n");
                for (i, key) in keys.iter().enumerate() {
                    out.push_str(&pad(depth + 1));
                    out.push_str(&escape(key));
                    out.push_str(": ");
                    let child_ty = if depth == 0 {
                        field_type(fields, key)
                    } else {
                        None
                    };
                    if let Some(child) = map.get(*key) {
                        write(child, fields, child_ty, depth + 1, pretty, out);
                    }
                    out.push_str(if i + 1 < keys.len() { ",\n" } else { "\n" });
                }
                out.push_str(&pad(depth));
                out.push('}');
            } else {
                out.push('{');
                for (i, key) in keys.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(&escape(key));
                    out.push_str(": ");
                    let child_ty = if depth == 0 {
                        field_type(fields, key)
                    } else {
                        None
                    };
                    if let Some(child) = map.get(*key) {
                        write(child, fields, child_ty, depth + 1, pretty, out);
                    }
                }
                out.push('}');
            }
        }
        // `Value` is non-exhaustive: geometry, ranges, record links and
        // array cells print through their own serialization.
        other => {
            let plain = expanded(other);
            if plain.is_null() {
                out.push_str("null");
            } else {
                write(&plain, fields, None, depth, pretty, out);
            }
        }
    }
}

/// The record as indented JSON, schema fields first.
pub fn pretty(value: &Value, fields: &[SchemaField]) -> String {
    let mut out = String::new();
    write(value, fields, None, 0, true, &mut out);
    out
}

/// The value on one line.
pub fn compact(value: &Value, fields: &[SchemaField]) -> String {
    let mut out = String::new();
    write(value, fields, None, 0, false, &mut out);
    out
}

/// A decimal as exact text, never through a float: padded to `scale`
/// places, or as typed without a scale (a bare DECIMAL). Text with more
/// places than `scale` stays as typed: rounding it would hide the error.
/// `None` when the text is not a plain decimal such as `-12.50`.
pub fn decimal_text(text: &str, scale: Option<u8>) -> Option<String> {
    let t = text.trim();
    let digits = t.strip_prefix('-').unwrap_or(t);
    let (int, frac) = digits.split_once('.').unwrap_or((digits, ""));
    let plain = !(int.is_empty() && frac.is_empty())
        && int.bytes().chain(frac.bytes()).all(|b| b.is_ascii_digit());
    if !plain {
        return None;
    }
    match scale.map(usize::from) {
        Some(scale) if frac.len() <= scale => {
            let sign = if t.starts_with('-') { "-" } else { "" };
            let int = if int.is_empty() { "0" } else { int };
            Some(
                format!("{sign}{int}.{frac:0<scale$}")
                    .trim_end_matches('.')
                    .to_string(),
            )
        }
        Some(_) | None => Some(t.to_string()),
    }
}

/// Brings a parsed value to the form the server stores: decimals as exact
/// text at the schema scale, timestamps as timestamps. Applied to both sides
/// of a diff, so formatting never reads as a change.
pub fn normalize(value: &Value, fields: &[SchemaField]) -> Value {
    let Value::Object(map) = value else {
        return value.clone();
    };
    let mut out = map.clone();
    for f in fields {
        let Some(v) = out.get(&f.name).cloned() else {
            continue;
        };
        let as_decimal = |text: String, scale: Option<u8>| {
            Value::String(decimal_text(&text, scale).unwrap_or(text))
        };
        let fixed = match (&f.ty, &v) {
            (FieldType::Decimal { scale, .. }, Value::String(s)) => {
                decimal_text(s, *scale).map(Value::String).unwrap_or(v)
            }
            (FieldType::Decimal { scale, .. }, Value::Float(n)) => {
                as_decimal(n.to_string(), *scale)
            }
            (FieldType::Decimal { scale, .. }, Value::Integer(n)) => {
                as_decimal(n.to_string(), *scale)
            }
            (FieldType::Decimal { scale, .. }, Value::Decimal(d)) => {
                as_decimal(d.to_string(), *scale)
            }
            (FieldType::Timestamp | FieldType::Datetime, Value::String(s)) => {
                NdbDateTime::parse(s).map(Value::DateTime).unwrap_or(v)
            }
            (FieldType::Float64, Value::Integer(n)) => Value::Float(*n as f64),
            _ => v,
        };
        out.insert(f.name.clone(), fixed);
    }
    Value::Object(out)
}

/// Parses editor text into an object, normalized against `fields`.
pub fn parse_object(text: &str, fields: &[SchemaField]) -> Result<Value, JsonError> {
    let parsed: Value = sonic_rs::from_str(text)
        .map(from_json)
        .map_err(|e| JsonError {
            message: format!("Invalid JSON: {e}"),
        })?;
    match parsed {
        Value::Object(_) => Ok(normalize(&parsed, fields)),
        _ => Err(JsonError {
            message: "A record is a JSON object: start with `{`.".to_string(),
        }),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    Key,
    Str,
    Num,
    Literal,
    Plain,
}

impl Token {
    /// The syntax class from `controls.css`.
    pub fn class(self) -> &'static str {
        match self {
            Token::Key => "tok-key",
            Token::Str => "tok-str",
            Token::Num => "tok-num",
            Token::Literal => "tok-kw",
            Token::Plain => "",
        }
    }
}

/// Splits one line of pretty JSON into colored spans.
pub fn tokens(line: &str) -> Vec<(Token, String)> {
    let chars: Vec<char> = line.chars().collect();
    let mut out: Vec<(Token, String)> = Vec::new();
    let mut i = 0;
    let push = |out: &mut Vec<(Token, String)>, t: Token, s: String| {
        if let Some((last, text)) = out.last_mut()
            && *last == t
            && t == Token::Plain
        {
            text.push_str(&s);
            return;
        }
        out.push((t, s));
    };
    while i < chars.len() {
        let c = chars[i];
        if c == '"' {
            let mut j = i + 1;
            while j < chars.len() && chars[j] != '"' {
                if chars[j] == '\\' {
                    j += 1;
                }
                j += 1;
            }
            let end = (j + 1).min(chars.len());
            let text: String = chars[i..end].iter().collect();
            let key = chars[end..].iter().find(|c| !c.is_whitespace()) == Some(&':');
            push(&mut out, if key { Token::Key } else { Token::Str }, text);
            i = end;
        } else if c.is_ascii_digit()
            || (c == '-' && chars.get(i + 1).is_some_and(|n| n.is_ascii_digit()))
        {
            let mut j = i + 1;
            while j < chars.len()
                && (chars[j].is_ascii_digit() || matches!(chars[j], '.' | 'e' | 'E' | '+' | '-'))
            {
                j += 1;
            }
            push(&mut out, Token::Num, chars[i..j].iter().collect());
            i = j;
        } else if c.is_ascii_alphabetic() {
            let mut j = i;
            while j < chars.len() && chars[j].is_ascii_alphabetic() {
                j += 1;
            }
            let word: String = chars[i..j].iter().collect();
            let t = if matches!(word.as_str(), "true" | "false" | "null") {
                Token::Literal
            } else {
                Token::Plain
            };
            push(&mut out, t, word);
            i = j;
        } else {
            push(&mut out, Token::Plain, c.to_string());
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn field(name: &str, ty: FieldType) -> SchemaField {
        SchemaField {
            name: name.into(),
            ty,
            required: false,
            reference: None,
            time_key: false,
        }
    }

    fn object(pairs: Vec<(&str, Value)>) -> Value {
        Value::Object(
            pairs
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect::<HashMap<_, _>>(),
        )
    }

    #[test]
    fn schema_fields_print_first_then_the_rest_alphabetically() {
        let v = object(vec![
            ("zeta", Value::Integer(1)),
            ("name", Value::String("Hana".into())),
            ("id", Value::String("u1".into())),
            ("alpha", Value::Bool(true)),
        ]);
        let fields = [
            field("id", FieldType::String { max: None }),
            field("name", FieldType::String { max: None }),
        ];
        let text = pretty(&v, &fields);
        let keys: Vec<&str> = text
            .lines()
            .filter_map(|l| l.trim().split('"').nth(1))
            .collect();
        assert_eq!(keys, ["id", "name", "alpha", "zeta"]);
    }

    #[test]
    fn decimals_print_as_bare_numbers_and_normalize_to_scale() {
        let f = [field(
            "total",
            FieldType::Decimal {
                precision: Some(10),
                scale: Some(2),
            },
        )];
        let v = object(vec![("total", Value::String("89.50".into()))]);
        assert!(pretty(&v, &f).contains("\"total\": 89.50"));
        let parsed = parse_object("{\"total\": 89.5}", &f).expect("parses");
        assert_eq!(compact(&parsed, &f), compact(&v, &f));
    }

    /// Every decimal the editor shows reads back as the stored value. A
    /// JSON number is an `f64`, so a decimal with more digits than an `f64`
    /// holds shows quoted.
    #[test]
    fn a_decimal_reads_back_exact_whatever_its_digits() {
        let fixed = field(
            "d",
            FieldType::Decimal {
                precision: Some(38),
                scale: Some(2),
            },
        );
        let free = field(
            "d",
            FieldType::Decimal {
                precision: None,
                scale: None,
            },
        );
        for (f, stored, shown) in [
            (&fixed, "89.50", "89.50"),
            (
                &fixed,
                "12345678901234567890.12",
                "\"12345678901234567890.12\"",
            ),
            (&free, "3.14159", "3.14159"),
            (&free, "1.50", "\"1.50\""),
            (
                &free,
                "12345678901234567890.123456",
                "\"12345678901234567890.123456\"",
            ),
        ] {
            let fields = [f.clone()];
            for v in [
                Value::String(stored.into()),
                Value::Decimal(stored.parse().expect("a decimal")),
            ] {
                let v = object(vec![("d", v)]);
                let text = compact(&v, &fields);
                assert_eq!(text, format!("{{\"d\": {shown}}}"), "{stored}");
                let back = parse_object(&text, &fields).expect("parses");
                assert_eq!(
                    compact(&normalize(&v, &fields), &fields),
                    compact(&back, &fields),
                    "{stored}"
                );
                assert_eq!(
                    normalize(&v, &fields),
                    object(vec![("d", Value::String(stored.into()))]),
                    "{stored} normalizes to its own text"
                );
            }
        }
        assert_eq!(decimal_text("7", Some(2)).as_deref(), Some("7.00"));
        assert_eq!(decimal_text("-.5", Some(2)).as_deref(), Some("-0.50"));
        assert_eq!(decimal_text("1.234", Some(2)).as_deref(), Some("1.234"));
        assert_eq!(decimal_text("7", Some(0)).as_deref(), Some("7"));
        for bad in ["NaN", "inf", "1e5", "", "-", ".", "1.2.3", "+1"] {
            assert_eq!(decimal_text(bad, Some(2)), None, "{bad}");
        }
    }

    /// JSON has no bytes, so an array of small numbers is an array.
    #[test]
    fn an_array_of_small_numbers_parses_as_an_array() {
        assert!(
            matches!(sonic_rs::from_str::<Value>("[1, 2]"), Ok(Value::Bytes(_))),
            "`Value` alone reads it as bytes: the reason for `from_json`"
        );
        let v = parse_object("{\"tags\": [1, 2, 255], \"o\": {\"a\": [0]}}", &[]).expect("parses");
        assert_eq!(
            v,
            object(vec![
                (
                    "tags",
                    Value::Array(vec![
                        Value::Integer(1),
                        Value::Integer(2),
                        Value::Integer(255)
                    ])
                ),
                (
                    "o",
                    object(vec![("a", Value::Array(vec![Value::Integer(0)]))])
                ),
            ])
        );
    }

    #[test]
    fn timestamps_round_trip_through_text() {
        let f = [field("at", FieldType::Timestamp)];
        let at = NdbDateTime::from_secs(1_781_356_860).expect("valid");
        let v = object(vec![("at", Value::DateTime(at))]);
        let text = pretty(&v, &f);
        assert!(text.contains("\"2026-06-13T13:21:00Z\""), "{text}");
        let back = parse_object(&text, &f).expect("parses");
        assert_eq!(compact(&back, &f), compact(&v, &f));
    }

    #[test]
    fn non_objects_and_bad_text_are_errors() {
        assert!(parse_object("[1]", &[]).is_err());
        assert!(parse_object("{\"a\": ", &[]).is_err());
    }

    #[test]
    fn strings_escape_quotes_and_control_characters() {
        let v = object(vec![("s", Value::String("a\"b\n".into()))]);
        assert!(compact(&v, &[]).contains("\"a\\\"b\\n\""));
    }

    #[test]
    fn tokens_mark_keys_strings_numbers_and_literals() {
        let t = tokens("  \"n\": 12, \"b\": true, \"s\": \"x\"");
        let kinds: Vec<Token> = t.iter().map(|(k, _)| *k).collect();
        assert!(kinds.contains(&Token::Key));
        assert!(kinds.contains(&Token::Num));
        assert!(kinds.contains(&Token::Literal));
        assert!(kinds.contains(&Token::Str));
    }

    #[test]
    fn a_vector_prints_its_head_and_length() {
        let v = object(vec![(
            "e",
            Value::Vector(vec![0.5, 0.25, 0.125, 0.0].into()),
        )]);
        assert!(compact(&v, &[]).contains("4 values"));
    }
}
