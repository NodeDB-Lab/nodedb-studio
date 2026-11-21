//! Record writes built from validated parts: insert, update and delete.
//!
//! An update writes only the fields the editor's review shows as changed,
//! decided the way the review decides it. An update and a delete also carry
//! in their `WHERE` the stored value of each field they depend on, so a
//! change another writer makes after the read matches no row. No function
//! here touches the network.

use std::collections::HashMap;

use nodedb_types::Value;

use crate::models::collection::Profile;
use crate::models::record_check::changes;
use crate::models::record_json::{compact, normalize};
use crate::models::schema::FieldType;
use crate::services::error::StudioError;
use crate::services::nodedb::explorer_sql::{Binder, Meta, Statement, id_text, rejected};

/// A value as the server binds it faithfully. The server stores an object as
/// JSON text, so an object binds as that text. A bound object reads back as a
/// string of non-JSON text, and a bound geometry as debug text.
pub(super) fn bindable(value: &Value) -> Value {
    match value {
        Value::Object(_) | Value::Geometry(_) => Value::String(compact(value, &[])),
        Value::Array(items) => Value::Array(items.iter().map(bindable).collect()),
        other => other.clone(),
    }
}

/// The fields of a record value, in name order, each name checked.
fn assignments(meta: &Meta, value: &Value) -> Result<Vec<(String, Value)>, StudioError> {
    let Value::Object(map) = value else {
        return Err(rejected("a record is an object of fields".to_string()));
    };
    let mut pairs: Vec<(String, Value)> = Vec::with_capacity(map.len());
    for (name, v) in map {
        pairs.push((meta.field(name)?.to_string(), v.clone()));
    }
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(pairs)
}

/// The key a record value carries must equal the key it is addressed by.
fn same_key(pairs: &[(String, Value)], key: &str, id: &str) -> Result<(), StudioError> {
    match pairs.iter().find(|(n, _)| n == key) {
        Some((_, v)) if id_text(v).as_deref() != Some(id) => Err(rejected(format!(
            "the record names `{key}` differently from `{id}`: a key cannot change"
        ))),
        _ => Ok(()),
    }
}

fn column_list(pairs: &[(String, Value)]) -> String {
    pairs
        .iter()
        .map(|(n, _)| format!("\"{n}\""))
        .collect::<Vec<_>>()
        .join(", ")
}

fn insert_like(verb: &str, meta: &Meta, pairs: Vec<(String, Value)>, returning: bool) -> Statement {
    let mut binder = Binder::default();
    let columns = column_list(&pairs);
    let marks: Vec<String> = pairs
        .iter()
        .map(|(_, v)| binder.push(bindable(v)))
        .collect();
    let tail = if returning { " RETURNING *" } else { "" };
    Statement {
        sql: format!(
            "{verb} INTO {} ({columns}) VALUES ({}){tail}",
            meta.collection,
            marks.join(", ")
        ),
        params: binder.params,
    }
}

pub(super) fn insert_statement(
    meta: &Meta,
    id: Option<&str>,
    value: &Value,
) -> Result<Statement, StudioError> {
    let mut pairs = assignments(meta, value)?;
    if let Some(id) = id {
        let key = meta.key()?.to_string();
        same_key(&pairs, &key, id)?;
        pairs.retain(|(n, _)| *n != key);
        pairs.push((key, meta.id_value(id)?));
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
    }
    if pairs.is_empty() {
        return Err(rejected("a record needs at least one field".to_string()));
    }
    Ok(insert_like("INSERT", meta, pairs, true))
}

/// Text that stands for a value the display cannot show in full
/// (`record_json::write`): bytes as `<N bytes>`, a vector as its first
/// components and its length. Written back, the text would replace the value.
fn placeholder(value: &Value) -> bool {
    let Value::String(s) = value else {
        return false;
    };
    let bytes = s
        .strip_prefix('<')
        .and_then(|r| r.strip_suffix(" bytes>"))
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
    let vector = s.starts_with('[') && s.ends_with(" values]") && s.contains('…');
    bytes || vector
}

/// The fields `new` sets differently from `stored`, decided as the editor's
/// review decides it (`record_check::changes`): both sides normalized against
/// the schema, then compared as compact text. A value the display shortens
/// equals its own display text, so an untouched bytes field is no change.
/// `None` writes NULL. NULL over an absent field is no change.
fn changed(
    meta: &Meta,
    key: &str,
    stored: &Value,
    new: &Value,
) -> Result<Vec<(String, Option<Value>)>, StudioError> {
    let fields = &meta.schema.fields;
    let Value::Object(draft) = normalize(new, fields) else {
        return Err(rejected("a record is an object of fields".to_string()));
    };
    let mut out = Vec::new();
    for change in changes(stored, new, fields) {
        if change.field == key {
            continue;
        }
        let value = draft
            .get(&change.field)
            .filter(|v| **v != Value::Null)
            .cloned();
        let was_null = change.before.as_deref().is_none_or(|b| b == "null");
        if value.is_none() && was_null {
            continue;
        }
        if let Some(v) = value.as_ref().filter(|v| placeholder(v)) {
            return Err(rejected(format!(
                "`{}` holds {}, the text Studio shows for a value it cannot display: \
                 Studio does not write it. Undo the change to `{}`.",
                change.field,
                compact(v, &[]),
                change.field
            )));
        }
        out.push((meta.field(&change.field)?.to_string(), value));
    }
    Ok(out)
}

/// `value` as a `WHERE` operand the server compares exactly, or `None`.
/// Checked live on 0.6.2: a scalar matches its stored value on every profile.
/// An object, an array or bytes does not match even its own stored value, so
/// it stays out of the condition.
fn operand(value: &Value) -> Option<Value> {
    match value {
        Value::Bool(_) | Value::Integer(_) | Value::String(_) => Some(value.clone()),
        Value::Float(f) if f.is_finite() => Some(value.clone()),
        Value::Uuid(s) | Value::Ulid(s) => Some(Value::String(s.clone())),
        Value::DateTime(t) | Value::NaiveDateTime(t) => Some(Value::DateTime(*t)),
        // Value is foreign and non-exhaustive: the arm is required.
        _ => None,
    }
}

/// Whether a field's declared type compares by value. A JSON field that holds
/// an object never matches it (checked live), so such a field stays out of the
/// condition whatever it holds.
fn compares(meta: &Meta, name: &str) -> bool {
    match meta.schema.field(name).map(|f| f.ty) {
        Some(
            FieldType::Object
            | FieldType::Array
            | FieldType::Json
            | FieldType::Vector { .. }
            | FieldType::Geometry { .. }
            | FieldType::Bytes,
        ) => false,
        Some(
            FieldType::String { .. }
            | FieldType::Int64
            | FieldType::Float64
            | FieldType::Decimal { .. }
            | FieldType::Bool
            | FieldType::Timestamp
            | FieldType::Datetime
            | FieldType::Uuid,
        )
        | None => true,
    }
}

/// `"f" = $n` or `"f" IS NULL` for each of `names` as `stored` holds it, so
/// the write lands only on a record whose fields still read that way. A value
/// that does not compare exactly stays out: the fingerprint check the caller
/// runs before the write still covers it.
fn unchanged_since(
    meta: &Meta,
    stored: &HashMap<String, Value>,
    names: &[&str],
    binder: &mut Binder,
) -> Vec<String> {
    names
        .iter()
        .filter(|n| compares(meta, n))
        .filter_map(|n| match stored.get(*n) {
            None | Some(Value::Null) => Some(format!("\"{n}\" IS NULL")),
            Some(v) => operand(v).map(|o| format!("\"{n}\" = {}", binder.push(o))),
        })
        .collect()
}

/// Whether a key-value entry holds a bare value. The server stores a lone
/// `value` column as raw bytes, not as a hash of fields, and refuses `UPDATE`
/// on it. Any other entry is a hash, which `UPDATE` merges into.
fn bare(stored: &HashMap<String, Value>, key: &str) -> bool {
    let mut names = stored.keys().filter(|n| *n != key);
    names.next().is_some_and(|n| n == "value") && names.next().is_none()
}

/// The statement that makes `stored` equal `new`, or `None` when the review
/// shows no change. `UPDATE` merges, so it sets only the changed fields, and
/// a field `new` lacks is set to NULL. A schemaless collection reads NULL as
/// absent.
pub(super) fn update_statement(
    meta: &Meta,
    id: &str,
    stored: &Value,
    new: &Value,
) -> Result<Option<Statement>, StudioError> {
    let key = meta.key()?;
    same_key(&assignments(meta, new)?, key, id)?;
    let Value::Object(old) = stored else {
        return Err(StudioError::MalformedRows {
            reason: "a stored record is not an object".to_string(),
        });
    };
    let set = changed(meta, key, stored, new)?;
    if set.is_empty() {
        return Ok(None);
    }
    if meta.profile == Profile::KeyValue && bare(old, key) {
        // `UPSERT` replaces the whole entry, so it carries every field: the
        // stored value of each one the edit leaves alone, the new value of
        // each it changes. It has no `WHERE`: only the fingerprint check
        // guards it.
        let mut all: HashMap<String, Value> = old
            .iter()
            .filter(|(n, _)| n.as_str() != key)
            .map(|(n, v)| (n.clone(), v.clone()))
            .collect();
        for (name, value) in set {
            match value {
                Some(v) => all.insert(name, v),
                None => all.remove(&name),
            };
        }
        let mut pairs = all
            .into_iter()
            .map(|(n, v)| Ok((meta.field(&n)?.to_string(), v)))
            .collect::<Result<Vec<_>, StudioError>>()?;
        pairs.push((key.to_string(), meta.id_value(id)?));
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        return Ok(Some(insert_like("UPSERT", meta, pairs, false)));
    }
    let mut binder = Binder::default();
    let assigns: Vec<String> = set
        .iter()
        .map(|(n, v)| match v {
            Some(v) => format!("\"{n}\" = {}", binder.push(bindable(v))),
            None => format!("\"{n}\" = NULL"),
        })
        .collect();
    let mut filter = vec![format!("\"{key}\" = {}", binder.push(meta.id_value(id)?))];
    let names: Vec<&str> = set.iter().map(|(n, _)| n.as_str()).collect();
    filter.extend(unchanged_since(meta, old, &names, &mut binder));
    Ok(Some(Statement {
        sql: format!(
            "UPDATE {} SET {} WHERE {}",
            meta.collection,
            assigns.join(", "),
            filter.join(" AND ")
        ),
        params: binder.params,
    }))
}

/// Removes record `id` only while every comparable field still holds what
/// `stored` holds. A stored field whose name Studio cannot put in a statement
/// stays out of the condition.
pub(super) fn delete_statement(
    meta: &Meta,
    id: &str,
    stored: &Value,
) -> Result<Statement, StudioError> {
    let key = meta.key()?;
    let mut binder = Binder::default();
    let mut filter = vec![format!("\"{key}\" = {}", binder.push(meta.id_value(id)?))];
    if let Value::Object(old) = stored {
        let mut names: Vec<&str> = old
            .keys()
            .map(String::as_str)
            .filter(|n| *n != key && meta.field(n).is_ok())
            .collect();
        names.sort_unstable();
        filter.extend(unchanged_since(meta, old, &names, &mut binder));
    }
    Ok(Statement {
        sql: format!(
            "DELETE FROM {} WHERE {}",
            meta.collection,
            filter.join(" AND ")
        ),
        params: binder.params,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::schema::CollectionSchema;
    use crate::services::nodedb::explorer_sql::tests::{field, obj, s, schemaless, strict};
    use nodedb_types::NdbDateTime;

    fn kv(fields: &[&str]) -> Meta {
        let mut schema = CollectionSchema::new("kv");
        schema.id_field = Some("key".into());
        schema.typed_headers = true;
        schema.fields = std::iter::once("key")
            .chain(fields.iter().copied())
            .map(|n| field(n, FieldType::String { max: None }))
            .collect();
        Meta {
            collection: "kv".into(),
            profile: Profile::KeyValue,
            schema,
        }
    }

    fn update(meta: &Meta, stored: &Value, new: &Value) -> Option<Statement> {
        update_statement(meta, "a", stored, new).expect("builds")
    }

    #[test]
    fn an_object_binds_as_json_text_and_never_enters_the_sql() {
        let value = obj(&[
            ("id", s("x'; DROP")),
            ("o", obj(&[("k", Value::Integer(1))])),
        ]);
        let st = insert_statement(&schemaless(), None, &value).expect("builds");
        assert_eq!(
            st.sql,
            "INSERT INTO d (\"id\", \"o\") VALUES ($1, $2) RETURNING *"
        );
        assert_eq!(st.params, [s("x'; DROP"), s("{\"k\": 1}")]);
        assert!(!st.sql.contains("DROP"));
    }

    #[test]
    fn an_insert_with_an_id_sets_the_key_and_refuses_a_different_one() {
        let st =
            insert_statement(&strict(), Some("k1"), &obj(&[("name", s("n"))])).expect("builds");
        assert_eq!(st.params, [s("k1"), s("n")]);
        let clash = insert_statement(&strict(), Some("k1"), &obj(&[("id", s("k2"))]));
        assert!(matches!(clash, Err(StudioError::Rejected { .. })));
        assert!(insert_statement(&strict(), None, &obj(&[])).is_err());
        assert!(insert_statement(&strict(), None, &Value::Null).is_err());
        assert!(insert_statement(&strict(), None, &obj(&[("zz", s("n"))])).is_err());
    }

    #[test]
    fn an_update_sets_changed_fields_only_while_they_read_as_read() {
        let stored = obj(&[
            ("id", s("a")),
            ("name", s("alice")),
            ("n", Value::Integer(1)),
        ]);
        let new = obj(&[("id", s("a")), ("name", s("alice2"))]);
        let st = update(&schemaless(), &stored, &new).expect("changes");
        assert_eq!(
            st.sql,
            "UPDATE d SET \"n\" = NULL, \"name\" = $1 \
             WHERE \"id\" = $2 AND \"n\" = $3 AND \"name\" = $4"
        );
        assert_eq!(
            st.params,
            [s("alice2"), s("a"), Value::Integer(1), s("alice")]
        );
        assert_eq!(update(&schemaless(), &stored, &stored), None);
        // an explicit null over an absent field is no change
        let nulled = obj(&[
            ("id", s("a")),
            ("name", s("alice")),
            ("n", Value::Integer(1)),
            ("x", Value::Null),
        ]);
        assert_eq!(update(&schemaless(), &stored, &nulled), None);
        // a field the record lacks is guarded as NULL
        let added = obj(&[
            ("id", s("a")),
            ("name", s("alice")),
            ("n", Value::Integer(1)),
            ("x", s("new")),
        ]);
        let st = update(&schemaless(), &stored, &added).expect("changes");
        assert!(
            st.sql.ends_with("WHERE \"id\" = $2 AND \"x\" IS NULL"),
            "{}",
            st.sql
        );
        let renamed = obj(&[("id", s("b"))]);
        assert!(update_statement(&schemaless(), "a", &stored, &renamed).is_err());
    }

    #[test]
    fn a_hash_entry_merges_and_a_bare_value_upserts_every_field() {
        // two value columns: an UPSERT of the changed one would erase the other
        let meta = kv(&["name", "email"]);
        let stored = obj(&[("key", s("a")), ("name", s("n1")), ("email", s("e1"))]);
        let new = obj(&[("key", s("a")), ("name", s("n2")), ("email", s("e1"))]);
        let st = update(&meta, &stored, &new).expect("changes");
        assert_eq!(
            st.sql,
            "UPDATE kv SET \"name\" = $1 WHERE \"key\" = $2 AND \"name\" = $3"
        );

        let meta = kv(&["value"]);
        let stored = obj(&[("key", s("a")), ("value", s("v"))]);
        let new = obj(&[("key", s("a")), ("value", s("w"))]);
        let st = update(&meta, &stored, &new).expect("changes");
        assert_eq!(
            st.sql,
            "UPSERT INTO kv (\"key\", \"value\") VALUES ($1, $2)"
        );
        assert_eq!(st.params, [s("a"), s("w")]);
    }

    #[test]
    fn a_field_the_review_shows_unchanged_is_not_written() {
        let mut meta = strict();
        meta.schema.fields.push(field("b", FieldType::Bytes));
        meta.schema.fields.push(field("at", FieldType::Timestamp));
        let at = NdbDateTime::from_micros(1_781_356_860_000_001);
        let stored = obj(&[
            ("id", s("a")),
            ("name", s("n")),
            ("b", Value::Bytes(vec![1; 16])),
            ("at", Value::NaiveDateTime(at)),
        ]);
        // the editor's draft: bytes as their display text, the time parsed
        let draft = obj(&[
            ("id", s("a")),
            ("name", s("n2")),
            ("b", s("<16 bytes>")),
            ("at", Value::DateTime(at)),
        ]);
        let st = update(&meta, &stored, &draft).expect("changes");
        assert_eq!(
            st.sql,
            "UPDATE c SET \"name\" = $1 WHERE \"id\" = $2 AND \"name\" = $3"
        );

        // a schemaless time or uuid reads back as text equal to the draft's
        let stored = obj(&[
            ("id", s("a")),
            ("at", Value::DateTime(at)),
            ("u", Value::Uuid("u-1".into())),
        ]);
        let draft = obj(&[
            ("id", s("a")),
            ("at", s("2026-06-13T13:21:00.000001Z")),
            ("u", s("u-1")),
        ]);
        assert_eq!(update(&schemaless(), &stored, &draft), None);
    }

    #[test]
    fn display_text_for_bytes_or_a_vector_is_never_written() {
        let stored = obj(&[("id", s("a")), ("b", Value::Bytes(vec![1; 16]))]);
        for text in ["<3 bytes>", "[0.5, 0.25, 0.125, … 768 values]"] {
            let draft = obj(&[("id", s("a")), ("b", s(text))]);
            match update_statement(&schemaless(), "a", &stored, &draft) {
                Err(StudioError::Rejected { reason }) => {
                    assert!(reason.contains("`b`"), "{reason}")
                }
                other => panic!("expected a refusal for {text}, got {other:?}"),
            }
        }
        assert!(!placeholder(&s("<x bytes>")) && !placeholder(&s("bytes")));
    }

    #[test]
    fn only_exactly_comparable_values_guard_a_write() {
        let mut meta = strict();
        meta.schema.fields.push(field("meta", FieldType::Json));
        meta.schema.fields.push(field("f", FieldType::Float64));
        let stored = obj(&[
            ("id", s("a")),
            ("name", s("n")),
            ("qty", Value::Null),
            ("meta", s("text in a JSON field")),
            ("f", Value::Float(f64::NAN)),
        ]);
        let st = delete_statement(&meta, "a", &stored).expect("builds");
        assert_eq!(
            st.sql,
            "DELETE FROM c WHERE \"id\" = $1 AND \"name\" = $2 AND \"qty\" IS NULL"
        );
        assert_eq!(st.params, [s("a"), s("n")]);

        let doc = obj(&[
            ("id", s("a")),
            ("o", obj(&[("k", Value::Integer(1))])),
            ("arr", Value::Array(vec![Value::Integer(1)])),
            ("at", Value::NaiveDateTime(NdbDateTime::from_micros(7))),
            ("u", Value::Uuid("u-1".into())),
            ("bad name", s("x")),
        ]);
        let st = delete_statement(&schemaless(), "a", &doc).expect("builds");
        assert_eq!(
            st.sql,
            "DELETE FROM d WHERE \"id\" = $1 AND \"at\" = $2 AND \"u\" = $3"
        );
        assert_eq!(
            st.params,
            [
                s("a"),
                Value::DateTime(NdbDateTime::from_micros(7)),
                s("u-1")
            ]
        );
    }

    #[test]
    fn a_numeric_key_deletes_by_number() {
        let mut meta = strict();
        meta.schema.fields[0] = field("id", FieldType::Int64);
        assert_eq!(
            delete_statement(&meta, "20", &Value::Null)
                .expect("builds")
                .params,
            [Value::Integer(20)]
        );
        let mut keyless = strict();
        keyless.schema.id_field = None;
        assert!(delete_statement(&keyless, "x", &Value::Null).is_err());
    }
}
