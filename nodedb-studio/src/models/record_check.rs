//! What a record edit changes, and whether the schema accepts it.

use std::collections::HashMap;

use nodedb_types::{NdbDateTime, Value};

use crate::models::record_json::{compact, normalize, pretty};
use crate::models::schema::{FieldType, SchemaField};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Removed,
    Modified,
}

impl ChangeKind {
    pub fn label(self) -> &'static str {
        match self {
            ChangeKind::Added => "Added",
            ChangeKind::Removed => "Removed",
            ChangeKind::Modified => "Modified",
        }
    }
}

/// One top-level field that differs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldChange {
    pub field: String,
    pub kind: ChangeKind,
    /// The stored value as text. `None` when the field is new.
    pub before: Option<String>,
    /// The proposed value as text. `None` when the field is removed.
    pub after: Option<String>,
}

fn entries(value: &Value) -> HashMap<&String, &Value> {
    match value {
        Value::Object(map) => map.iter().collect(),
        _ => HashMap::new(),
    }
}

/// The top-level fields that differ between `stored` and `draft`, schema
/// fields first. Values compare as the server stores them.
pub fn changes(stored: &Value, draft: &Value, fields: &[SchemaField]) -> Vec<FieldChange> {
    let stored = normalize(stored, fields);
    let draft = normalize(draft, fields);
    let (a, b) = (entries(&stored), entries(&draft));
    let mut names: Vec<&String> = a.keys().chain(b.keys()).copied().collect();
    names.sort();
    names.dedup();
    names.sort_by_key(|n| {
        fields
            .iter()
            .position(|f| &f.name == *n)
            .unwrap_or(usize::MAX)
    });
    let one = |v: &Value| compact(v, &[]);
    names
        .into_iter()
        .filter_map(|name| match (a.get(name), b.get(name)) {
            (Some(x), Some(y)) if one(x) == one(y) => None,
            (Some(x), Some(y)) => Some(FieldChange {
                field: name.clone(),
                kind: ChangeKind::Modified,
                before: Some(one(x)),
                after: Some(one(y)),
            }),
            (None, Some(y)) => Some(FieldChange {
                field: name.clone(),
                kind: ChangeKind::Added,
                before: None,
                after: Some(one(y)),
            }),
            (Some(x), None) => Some(FieldChange {
                field: name.clone(),
                kind: ChangeKind::Removed,
                before: Some(one(x)),
                after: None,
            }),
            (None, None) => None,
        })
        .collect()
}

/// One reason the schema rejects a record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub field: String,
    pub message: String,
}

fn type_error(ty: &FieldType, v: &Value) -> Option<String> {
    let bad = |want: &str| Some(format!("must be {want}"));
    match (ty, v) {
        (_, Value::Null) => None,
        (FieldType::String { max }, Value::String(s)) => max
            .filter(|m| s.chars().count() > *m as usize)
            .map(|m| format!("must be at most {m} characters")),
        (FieldType::String { .. }, _) => bad("a string"),
        (FieldType::Uuid, Value::String(_) | Value::Uuid(_)) => None,
        (FieldType::Uuid, _) => bad("a UUID"),
        (FieldType::Int64, Value::Integer(_)) => None,
        (FieldType::Int64, _) => bad("an integer"),
        (FieldType::Float64, Value::Float(_) | Value::Integer(_)) => None,
        (FieldType::Float64, _) => bad("a number"),
        (FieldType::Decimal { precision, scale }, Value::String(s)) => {
            let digits = s.trim_start_matches('-').replace('.', "").len();
            let frac = s.split_once('.').map_or(0, |(_, f)| f.len());
            if s.parse::<f64>().is_err() {
                bad("a decimal number")
            } else if frac > usize::from(*scale) || digits > usize::from(*precision) {
                Some(format!("must fit decimal({precision},{scale})"))
            } else {
                None
            }
        }
        (FieldType::Decimal { .. }, _) => bad("a decimal number"),
        (FieldType::Bool, Value::Bool(_)) => None,
        (FieldType::Bool, _) => bad("true or false"),
        (FieldType::Timestamp | FieldType::Datetime, Value::DateTime(_)) => None,
        (FieldType::Timestamp | FieldType::Datetime, _) => {
            bad("an ISO 8601 time, like 2026-06-13T12:41:02Z")
        }
        (FieldType::Object, Value::Object(_)) => None,
        (FieldType::Object, _) => bad("an object"),
        (FieldType::Array, Value::Array(_)) => None,
        (FieldType::Array, _) => bad("an array"),
        (FieldType::Json, _) => None,
        (FieldType::Vector { .. } | FieldType::Bytes, _) => None,
        (FieldType::Geometry { .. }, Value::Object(_) | Value::Geometry(_)) => None,
        (FieldType::Geometry { .. }, _) => bad("a GeoJSON geometry"),
    }
}

/// What a schema that enforces its fields rejects: a missing required
/// field, a wrong type, a field it does not declare. A flexible collection
/// accepts anything, so it passes `enforced: false`.
pub fn violations(enforced: bool, fields: &[SchemaField], value: &Value) -> Vec<Violation> {
    if !enforced {
        return Vec::new();
    }
    let value = normalize(value, fields);
    let map = entries(&value);
    let mut out = Vec::new();
    for f in fields {
        match map.get(&f.name) {
            None | Some(Value::Null) if f.required => out.push(Violation {
                field: f.name.clone(),
                message: "is required".to_string(),
            }),
            Some(v) => {
                if let Some(message) = type_error(&f.ty, v) {
                    out.push(Violation {
                        field: f.name.clone(),
                        message,
                    });
                }
            }
            None => {}
        }
    }
    let mut unknown: Vec<&&String> = map
        .keys()
        .filter(|k| !fields.iter().any(|f| &f.name == **k))
        .collect();
    unknown.sort();
    for k in unknown {
        out.push(Violation {
            field: (*k).clone(),
            message: "is not in the schema".to_string(),
        });
    }
    out
}

/// What a form input holds for `field`, as text.
pub fn input_text(value: Option<&Value>, ty: &FieldType) -> String {
    match (value, ty) {
        (None | Some(Value::Null), _) => String::new(),
        (Some(Value::String(s)), _) => s.clone(),
        (Some(Value::DateTime(t)), _) => crate::models::record_json::iso(t),
        (Some(v @ (Value::Object(_) | Value::Array(_))), _) => pretty(v, &[]),
        (Some(v), _) => compact(v, &[]),
    }
}

/// Sets one field from form input text and returns the new editor text.
/// An empty input removes an optional field.
pub fn set_field(
    text: &str,
    fields: &[SchemaField],
    name: &str,
    input: &str,
) -> Result<String, crate::models::record_json::JsonError> {
    let Value::Object(mut map) = crate::models::record_json::parse_object(text, fields)? else {
        return Ok(text.to_string());
    };
    let ty = fields.iter().find(|f| f.name == name).map(|f| f.ty);
    let value = if input.trim().is_empty() {
        None
    } else {
        Some(match ty {
            Some(FieldType::Int64) => input
                .trim()
                .parse::<i64>()
                .map(Value::Integer)
                .unwrap_or_else(|_| Value::String(input.to_string())),
            Some(FieldType::Float64) => input
                .trim()
                .parse::<f64>()
                .map(Value::Float)
                .unwrap_or_else(|_| Value::String(input.to_string())),
            Some(FieldType::Bool) => match input.trim() {
                "true" => Value::Bool(true),
                "false" => Value::Bool(false),
                _ => Value::String(input.to_string()),
            },
            Some(FieldType::Timestamp | FieldType::Datetime) => NdbDateTime::parse(input.trim())
                .map(Value::DateTime)
                .unwrap_or_else(|| Value::String(input.to_string())),
            Some(FieldType::Object | FieldType::Array | FieldType::Json) => {
                sonic_rs::from_str::<Value>(input)
                    .unwrap_or_else(|_| Value::String(input.to_string()))
            }
            Some(FieldType::String { .. } | FieldType::Decimal { .. } | FieldType::Uuid) | None => {
                Value::String(input.to_string())
            }
            Some(FieldType::Vector { .. } | FieldType::Geometry { .. } | FieldType::Bytes) => {
                Value::String(input.to_string())
            }
        })
    };
    match value {
        Some(v) => map.insert(name.to_string(), v),
        None => map.remove(name),
    };
    Ok(pretty(&Value::Object(map), fields))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(name: &str, ty: FieldType, required: bool) -> SchemaField {
        SchemaField {
            name: name.into(),
            ty,
            required,
            reference: None,
            time_key: false,
        }
    }

    fn obj(pairs: Vec<(&str, Value)>) -> Value {
        Value::Object(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }

    #[test]
    fn only_differing_fields_change() {
        let a = obj(vec![
            ("status", Value::String("pending".into())),
            ("n", Value::Integer(1)),
        ]);
        let b = obj(vec![
            ("status", Value::String("active".into())),
            ("n", Value::Integer(1)),
        ]);
        let c = changes(&a, &b, &[]);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].field, "status");
        assert_eq!(c[0].kind, ChangeKind::Modified);
        assert_eq!(c[0].before.as_deref(), Some("\"pending\""));
        assert_eq!(c[0].after.as_deref(), Some("\"active\""));
    }

    #[test]
    fn added_and_removed_fields_are_reported() {
        let a = obj(vec![("old", Value::Bool(true))]);
        let b = obj(vec![("new", Value::Bool(true))]);
        let kinds: Vec<ChangeKind> = changes(&a, &b, &[]).iter().map(|c| c.kind).collect();
        assert_eq!(kinds, [ChangeKind::Added, ChangeKind::Removed]);
    }

    #[test]
    fn a_strict_schema_names_each_violation() {
        let fields = [
            f("id", FieldType::String { max: None }, true),
            f(
                "total",
                FieldType::Decimal {
                    precision: 10,
                    scale: 2,
                },
                true,
            ),
            f("currency", FieldType::String { max: Some(3) }, true),
        ];
        let v = obj(vec![
            ("total", Value::String("12.345".into())),
            ("currency", Value::String("USDX".into())),
            ("extra", Value::Bool(true)),
        ]);
        let found: Vec<(String, String)> = violations(true, &fields, &v)
            .into_iter()
            .map(|x| (x.field, x.message))
            .collect();
        assert!(found.contains(&("id".into(), "is required".into())));
        assert!(
            found
                .iter()
                .any(|(n, m)| n == "total" && m.contains("decimal(10,2)"))
        );
        assert!(
            found
                .iter()
                .any(|(n, m)| n == "currency" && m.contains("at most 3"))
        );
        assert!(found.contains(&("extra".into(), "is not in the schema".into())));
    }

    #[test]
    fn a_flexible_schema_accepts_anything() {
        let fields = [f("id", FieldType::String { max: None }, true)];
        assert!(violations(false, &fields, &obj(vec![("x", Value::Null)])).is_empty());
    }

    #[test]
    fn a_form_edit_rewrites_the_text() {
        let fields = [
            f("n", FieldType::Int64, false),
            f("s", FieldType::String { max: None }, false),
        ];
        let text = "{\"n\": 1, \"s\": \"a\"}";
        let out = set_field(text, &fields, "n", "42").expect("edits");
        assert!(out.contains("\"n\": 42"), "{out}");
        let out = set_field(&out, &fields, "s", "").expect("removes");
        assert!(!out.contains("\"s\""), "{out}");
    }
}
