//! Record writes built from validated parts: insert, update and delete.
//!
//! An update writes only the fields the editor's review shows as changed,
//! decided the way the review decides it. An update and a delete also carry
//! in their `WHERE` the stored value of each field they depend on, so a
//! change another writer makes after the read matches no row. Every value
//! written passes `check_written`: no bytes, and no display placeholder. No
//! function here touches the network.

use std::collections::HashMap;

use nodedb_types::Value;

use crate::models::collection::Profile;
use crate::models::records::check::changes;
use crate::models::records::json::{compact, normalize};
use crate::models::schema::FieldType;
use crate::services::error::StudioError;
use crate::services::nodedb::explorer::sql::{Binder, Meta, Statement, id_text, rejected};

/// A value as the server binds it faithfully. The server stores an object as
/// JSON text, so an object binds as that text. A bound object reads back as a
/// string of non-JSON text, and a bound geometry as debug text.
pub(in crate::services::nodedb) fn bindable(value: &Value) -> Value {
    match value {
        Value::Object(_) | Value::Geometry(_) => Value::String(compact(value, &[])),
        Value::Array(items) => Value::Array(items.iter().map(bindable).collect()),
        other => other.clone(),
    }
}

/// A write that lands only while the fields in `guards` still read as the
/// caller read them.
#[derive(Debug, Clone, PartialEq)]
pub(in crate::services::nodedb) struct Guarded {
    pub statement: Statement,
    /// The fields the `WHERE` compares, the key aside.
    pub guards: Vec<String>,
}

/// Why Studio writes no bytes. Checked live on 0.6.2: 16 bytes bound over
/// the native protocol read back as 34 bytes of `\x..` text from a BYTES
/// column, and as text from a schemaless field or a key-value entry.
// ponytail: lift this refusal when `live_explorer_bytes_do_not_round_trip`
// fails, which means the server stores bytes as written.
const NO_BYTES: &str = "NodeDB 0.6.2 does not store bytes correctly over this protocol: \
     they read back as different bytes or as text. Studio does not write bytes to this server.";

/// The first value in `value`, itself included, that `hit` picks, searched
/// through arrays, sets and object fields.
fn find<'a>(value: &'a Value, hit: &dyn Fn(&Value) -> bool) -> Option<&'a Value> {
    if hit(value) {
        return Some(value);
    }
    match value {
        Value::Array(items) | Value::Set(items) => items.iter().find_map(|v| find(v, hit)),
        Value::Object(map) => map.values().find_map(|v| find(v, hit)),
        // Value is foreign and non-exhaustive: the arm is required.
        _ => None,
    }
}

/// Text that stands for a value the display cannot show in full
/// (`records::json::write`): bytes as `<N bytes>`, a vector as its first
/// components and its length. Written back, the text would replace the value.
/// The display's other forms read back whole: a decimal an `f64` cannot hold
/// shows quoted, and a NaN or infinite float shows as `null`, which writes
/// NULL as shown.
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

/// Refuses `value` for field `name` when the server would not store it as
/// the editor shows it: bytes anywhere in it, a non-NULL value for a field
/// declared as bytes, or a display placeholder anywhere in it. `fix` tells
/// the user what to do instead.
fn check_written(meta: &Meta, name: &str, value: &Value, fix: &str) -> Result<(), StudioError> {
    let declared_bytes = matches!(
        meta.schema.field(name).map(|f| f.ty),
        Some(FieldType::Bytes)
    );
    if find(value, &|v| matches!(v, Value::Bytes(_))).is_some() {
        return Err(rejected(format!("`{name}` holds bytes. {NO_BYTES} {fix}")));
    }
    if declared_bytes && *value != Value::Null {
        return Err(rejected(format!(
            "`{name}` is a BYTES field, so the server stores any value written to it as bytes. \
             {NO_BYTES} {fix}"
        )));
    }
    match find(value, &placeholder) {
        Some(text) => Err(rejected(format!(
            "`{name}` holds {}, the text Studio shows for a value it cannot display: \
             Studio does not write it. {fix}",
            compact(text, &[])
        ))),
        None => Ok(()),
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
fn same_key(value: &Value, key: &str, id: &str) -> Result<(), StudioError> {
    let Value::Object(map) = value else {
        return Err(rejected("a record is an object of fields".to_string()));
    };
    match map.get(key) {
        Some(v) if id_text(v).as_deref() != Some(id) => Err(rejected(format!(
            "the record names `{key}` differently from `{id}`: a key cannot change"
        ))),
        Some(_) | None => Ok(()),
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

/// Inserts every field of `value`, each one checked by `check_written`.
pub(in crate::services::nodedb) fn insert_statement(
    meta: &Meta,
    id: Option<&str>,
    value: &Value,
) -> Result<Statement, StudioError> {
    let mut pairs = assignments(meta, value)?;
    if let Some(id) = id {
        let key = meta.key()?.to_string();
        same_key(value, &key, id)?;
        pairs.retain(|(n, _)| *n != key);
        pairs.push((key, meta.id_value(id)?));
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
    }
    if pairs.is_empty() {
        return Err(rejected("a record needs at least one field".to_string()));
    }
    for (name, v) in &pairs {
        check_written(
            meta,
            name,
            v,
            &format!("Remove `{name}` from the record, or give it a value."),
        )?;
    }
    Ok(insert_like("INSERT", meta, pairs, true))
}

/// The fields `new` sets differently from `stored`, decided as the editor's
/// review decides it (`records::check::changes`): both sides normalized against
/// the schema, then compared as compact text. A value the display shortens
/// equals its own display text, so an untouched bytes field is no change.
/// `None` writes NULL. NULL over an absent field is no change. Only these
/// fields' names reach the statement, so a name Studio cannot write blocks
/// only an edit of that field.
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
        let name = meta.field(&change.field)?;
        if let Some(v) = &value {
            check_written(meta, name, v, &format!("Undo the change to `{name}`."))?;
        }
        out.push((name.to_string(), value));
    }
    Ok(out)
}

/// `value` as a `WHERE` operand the server compares exactly, or `None`.
/// Checked live on 0.6.2 on strict, schemaless, key-value and columnar
/// collections: text (quotes and non-ASCII included), integers, finite
/// floats, booleans, decimals (they read back as text) and TIMESTAMP and
/// TIMESTAMPTZ values each match their stored value. Left out: an object, an
/// array or bytes (no match even with its own stored value), a NaN or
/// infinite float, a UUID (a columnar UUID column does not match its own
/// text), and every other variant.
fn operand(value: &Value) -> Option<Value> {
    match value {
        Value::Bool(_) | Value::Integer(_) | Value::String(_) => Some(value.clone()),
        Value::Float(f) if f.is_finite() => Some(value.clone()),
        Value::DateTime(t) | Value::NaiveDateTime(t) => Some(Value::DateTime(*t)),
        // Value is foreign and non-exhaustive: the arm is required.
        _ => None,
    }
}

/// Whether a field's declared type compares by value. A JSON field that holds
/// an object never matches it, and a columnar UUID field never matches its
/// own text (both checked live), so such a field stays out of the condition
/// whatever it holds.
fn compares(meta: &Meta, name: &str) -> bool {
    match meta.schema.field(name).map(|f| f.ty) {
        Some(
            FieldType::Object
            | FieldType::Array
            | FieldType::Json
            | FieldType::Vector { .. }
            | FieldType::Geometry { .. }
            | FieldType::Bytes
            | FieldType::Uuid,
        ) => false,
        Some(
            FieldType::String { .. }
            | FieldType::Int64
            | FieldType::Float64
            | FieldType::Decimal { .. }
            | FieldType::Bool
            | FieldType::Timestamp
            | FieldType::Datetime,
        )
        | None => true,
    }
}

/// `"f" = $n` or `"f" IS NULL` for each of `names` as `stored` holds it, so
/// the write lands only on a record whose fields still read that way. A value
/// that does not compare exactly stays out: the fingerprint check the caller
/// runs before the write still covers it. Returns the guarded names and
/// their conditions.
fn unchanged_since(
    meta: &Meta,
    stored: &HashMap<String, Value>,
    names: &[&str],
    binder: &mut Binder,
) -> (Vec<String>, Vec<String>) {
    names
        .iter()
        .filter(|n| compares(meta, n))
        .filter_map(|n| match stored.get(*n) {
            None | Some(Value::Null) => Some((n.to_string(), format!("\"{n}\" IS NULL"))),
            Some(v) => operand(v).map(|o| (n.to_string(), format!("\"{n}\" = {}", binder.push(o)))),
        })
        .unzip()
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
pub(in crate::services::nodedb) fn update_statement(
    meta: &Meta,
    id: &str,
    stored: &Value,
    new: &Value,
) -> Result<Option<Guarded>, StudioError> {
    let key = meta.key()?;
    same_key(new, key, id)?;
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
        return Ok(Some(Guarded {
            statement: insert_like("UPSERT", meta, pairs, false),
            guards: Vec::new(),
        }));
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
    let (guards, conditions) = unchanged_since(meta, old, &names, &mut binder);
    filter.extend(conditions);
    Ok(Some(Guarded {
        statement: Statement {
            sql: format!(
                "UPDATE {} SET {} WHERE {}",
                meta.collection,
                assigns.join(", "),
                filter.join(" AND ")
            ),
            params: binder.params,
        },
        guards,
    }))
}

/// Removes record `id` only while every comparable field still holds what
/// `stored` holds. A stored field whose name Studio cannot put in a statement
/// stays out of the condition.
pub(in crate::services::nodedb) fn delete_statement(
    meta: &Meta,
    id: &str,
    stored: &Value,
) -> Result<Guarded, StudioError> {
    let key = meta.key()?;
    let mut binder = Binder::default();
    let mut filter = vec![format!("\"{key}\" = {}", binder.push(meta.id_value(id)?))];
    let mut guards = Vec::new();
    if let Value::Object(old) = stored {
        let mut names: Vec<&str> = old
            .keys()
            .map(String::as_str)
            .filter(|n| *n != key && meta.field(n).is_ok())
            .collect();
        names.sort_unstable();
        let (names, conditions) = unchanged_since(meta, old, &names, &mut binder);
        guards = names;
        filter.extend(conditions);
    }
    Ok(Guarded {
        statement: Statement {
            sql: format!(
                "DELETE FROM {} WHERE {}",
                meta.collection,
                filter.join(" AND ")
            ),
            params: binder.params,
        },
        guards,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::schema::CollectionSchema;
    use crate::services::nodedb::explorer::sql::tests::{field, obj, s, schemaless, strict};
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
        update_statement(meta, "a", stored, new)
            .expect("builds")
            .map(|g| g.statement)
    }

    fn refusal(result: Result<impl std::fmt::Debug, StudioError>) -> String {
        match result {
            Err(StudioError::Rejected { reason }) => reason,
            other => panic!("expected a refusal, got {other:?}"),
        }
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
        let g = delete_statement(&meta, "a", &stored).expect("builds");
        assert_eq!(
            g.statement.sql,
            "DELETE FROM c WHERE \"id\" = $1 AND \"name\" = $2 AND \"qty\" IS NULL"
        );
        assert_eq!(g.statement.params, [s("a"), s("n")]);
        assert_eq!(g.guards, ["name", "qty"]);

        let doc = obj(&[
            ("id", s("a")),
            ("o", obj(&[("k", Value::Integer(1))])),
            ("arr", Value::Array(vec![Value::Integer(1)])),
            ("at", Value::NaiveDateTime(NdbDateTime::from_micros(7))),
            ("u", Value::Uuid("u-1".into())),
            ("e-mail", s("x")),
            ("bad\"name", s("x")),
        ]);
        let g = delete_statement(&schemaless(), "a", &doc).expect("builds");
        assert_eq!(
            g.statement.sql,
            "DELETE FROM d WHERE \"id\" = $1 AND \"at\" = $2 AND \"e-mail\" = $3"
        );
        assert_eq!(
            g.statement.params,
            [s("a"), Value::DateTime(NdbDateTime::from_micros(7)), s("x")]
        );

        // a declared UUID never guards: a columnar UUID column does not
        // match its own text on 0.6.2
        let mut meta = strict();
        meta.schema.fields.push(field("u", FieldType::Uuid));
        let stored = obj(&[
            ("id", s("a")),
            ("u", s("6f1c2a3e-1b2c-4d5e-8f90-1234567890ab")),
        ]);
        let g = delete_statement(&meta, "a", &stored).expect("builds");
        assert_eq!(g.statement.sql, "DELETE FROM c WHERE \"id\" = $1");
        assert!(g.guards.is_empty());
    }

    /// An insert writes every field, so every field passes the checks, and
    /// a placeholder inside an array or an object is found too.
    #[test]
    fn an_insert_never_writes_display_text_anywhere_in_a_field() {
        let id = ("id", s("a2"));
        let vector = s("[0.5, 0.25, 0.125, … 768 values]");
        for (name, value) in [
            ("b", s("<16 bytes>")),
            (
                "arr",
                Value::Array(vec![Value::Integer(1), s("<16 bytes>")]),
            ),
            ("o", obj(&[("deep", obj(&[("b", s("<2 bytes>"))]))])),
            ("e", vector),
        ] {
            let reason = refusal(insert_statement(
                &schemaless(),
                None,
                &obj(&[id.clone(), (name, value)]),
            ));
            assert!(reason.contains(&format!("`{name}`")), "{reason}");
            assert!(reason.contains("Remove"), "{reason}");
        }
        // text that only resembles a placeholder is written
        assert!(insert_statement(&schemaless(), None, &obj(&[id, ("t", s("<x bytes>"))])).is_ok());
    }

    /// 0.6.2 corrupts bytes written over the native protocol, so no write
    /// carries them: not as a value, not inside one, and not into a field
    /// declared as bytes. NULL carries no bytes.
    #[test]
    fn no_write_carries_bytes() {
        let bytes = Value::Bytes(vec![1; 16]);
        for value in [
            obj(&[("id", s("a")), ("b", bytes.clone())]),
            obj(&[("id", s("a")), ("arr", Value::Array(vec![bytes.clone()]))]),
        ] {
            let reason = refusal(insert_statement(&schemaless(), None, &value));
            assert!(
                reason.contains("0.6.2") && reason.contains("bytes"),
                "{reason}"
            );
        }
        let mut meta = strict();
        meta.schema.fields.push(field("b", FieldType::Bytes));
        let reason = refusal(insert_statement(
            &meta,
            None,
            &obj(&[("id", s("a")), ("b", s("AAEC"))]),
        ));
        assert!(
            reason.contains("`b`") && reason.contains("0.6.2"),
            "{reason}"
        );
        assert!(insert_statement(&meta, None, &obj(&[("id", s("a")), ("b", Value::Null)])).is_ok());

        let stored = obj(&[("id", s("a")), ("b", bytes.clone()), ("name", s("n"))]);
        let reason = refusal(update_statement(
            &meta,
            "a",
            &stored,
            &obj(&[("id", s("a")), ("b", s("AAEC")), ("name", s("n"))]),
        ));
        assert!(reason.contains("Undo the change to `b`"), "{reason}");
        // removing the bytes writes NULL, which stores no bytes
        let st =
            update(&meta, &stored, &obj(&[("id", s("a")), ("name", s("n"))])).expect("changes");
        assert!(
            st.sql.starts_with("UPDATE c SET \"b\" = NULL WHERE"),
            "{}",
            st.sql
        );
    }

    /// An array that held bytes shows them as `<N bytes>`. Changing another
    /// element would write that text over the bytes.
    #[test]
    fn an_update_never_writes_display_text_inside_a_changed_field() {
        let stored = obj(&[
            ("id", s("a")),
            (
                "arr",
                Value::Array(vec![Value::Bytes(vec![1, 2]), Value::Integer(1)]),
            ),
        ]);
        let draft = obj(&[
            ("id", s("a")),
            ("arr", Value::Array(vec![s("<2 bytes>"), Value::Integer(2)])),
        ]);
        let reason = refusal(update_statement(&schemaless(), "a", &stored, &draft));
        assert!(reason.contains("Undo the change to `arr`"), "{reason}");
        // untouched, the same array is no change and is not written
        let same = obj(&[
            ("id", s("a")),
            ("arr", Value::Array(vec![s("<2 bytes>"), Value::Integer(1)])),
        ]);
        assert_eq!(update(&schemaless(), &stored, &same), None);
    }

    /// Only the key and the fields an edit writes must be names Studio can
    /// put in a statement. A quoted name with a dash works on 0.6.2. A name
    /// holding `"` does not, and blocks only an edit of that field.
    #[test]
    fn an_edit_checks_only_the_names_it_writes() {
        let stored = obj(&[
            ("id", s("a")),
            ("e-mail", s("x@y")),
            ("a\"q", s("q")),
            ("name", s("n")),
        ]);
        let with = |field: &str, value: &str| {
            let mut draft = stored.clone();
            if let Value::Object(m) = &mut draft {
                m.insert(field.into(), s(value));
            }
            draft
        };
        let st = update(&schemaless(), &stored, &with("name", "n2")).expect("changes");
        assert_eq!(
            st.sql,
            "UPDATE d SET \"name\" = $1 WHERE \"id\" = $2 AND \"name\" = $3"
        );
        let st = update(&schemaless(), &stored, &with("e-mail", "z@y")).expect("changes");
        assert_eq!(
            st.sql,
            "UPDATE d SET \"e-mail\" = $1 WHERE \"id\" = $2 AND \"e-mail\" = $3"
        );
        let reason = refusal(update_statement(
            &schemaless(),
            "a",
            &stored,
            &with("a\"q", "q2"),
        ));
        assert!(reason.contains("a\"q"), "{reason}");
        let g = delete_statement(&schemaless(), "a", &stored).expect("builds");
        assert_eq!(
            g.guards,
            ["e-mail", "name"],
            "the unwritable name stays out"
        );
    }

    #[test]
    fn a_numeric_key_deletes_by_number() {
        let mut meta = strict();
        meta.schema.fields[0] = field("id", FieldType::Int64);
        assert_eq!(
            delete_statement(&meta, "20", &Value::Null)
                .expect("builds")
                .statement
                .params,
            [Value::Integer(20)]
        );
        let mut keyless = strict();
        keyless.schema.id_field = None;
        assert!(delete_statement(&keyless, "x", &Value::Null).is_err());
    }
}
