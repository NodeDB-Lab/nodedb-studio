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
        .unwrap_or(Value::Null)
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
            Some(FieldType::Decimal { .. }) if s.parse::<f64>().is_ok() => out.push_str(s),
            _ => out.push_str(&escape(s)),
        },
        Value::Uuid(s) | Value::Ulid(s) | Value::Regex(s) => out.push_str(&escape(s)),
        Value::DateTime(t) | Value::NaiveDateTime(t) => out.push_str(&escape(&iso(t))),
        Value::Decimal(d) => out.push_str(&d.to_string()),
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

/// A decimal as exact text padded to `scale` places. Text with more places
/// than `scale` stays as typed: rounding it would hide the error. `None`
/// when the text is not a number.
fn decimal_text(text: &str, scale: u8) -> Option<String> {
    let n: f64 = text.trim().parse().ok()?;
    let places = text.trim().split_once('.').map_or(0, |(_, f)| f.len());
    if places > usize::from(scale) {
        Some(text.trim().to_string())
    } else {
        Some(format!("{n:.prec$}", prec = usize::from(scale)))
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
        let fixed = match (&f.ty, &v) {
            (FieldType::Decimal { scale, .. }, Value::String(s)) => {
                decimal_text(s, *scale).map(Value::String).unwrap_or(v)
            }
            (FieldType::Decimal { scale, .. }, Value::Float(n)) => {
                Value::String(decimal_text(&n.to_string(), *scale).unwrap_or_else(|| n.to_string()))
            }
            (FieldType::Decimal { scale, .. }, Value::Integer(n)) => {
                Value::String(format!("{n}.{}", "0".repeat(usize::from(*scale))))
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
    let parsed: Value = sonic_rs::from_str(text).map_err(|e| JsonError {
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
                precision: 10,
                scale: 2,
            },
        )];
        let v = object(vec![("total", Value::String("89.50".into()))]);
        assert!(pretty(&v, &f).contains("\"total\": 89.50"));
        let parsed = parse_object("{\"total\": 89.5}", &f).expect("parses");
        assert_eq!(compact(&parsed, &f), compact(&v, &f));
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
