//! Record SQL built from validated parts, and rows decoded into records.
//!
//! Values travel as `$n` parameters. Identifiers (collection, field names)
//! cannot be bound, so each one passes `Meta::field` or `ident` before it
//! reaches the text. No function here touches the network.

use std::collections::HashMap;

use nodedb_client::QueryResult;
use nodedb_types::{NdbDateTime, Value};

use crate::models::collection::Profile;
use crate::models::record_json::compact;
use crate::models::records::{Record, RecordQuery};
use crate::models::schema::{CollectionSchema, FieldType};
use crate::services::error::StudioError;

/// What a write or read needs to know about one collection: its storage
/// profile and its declared shape.
pub(super) struct Meta {
    pub collection: String,
    pub profile: Profile,
    pub schema: CollectionSchema,
}

/// A statement with its bound values, in `$n` order.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Statement {
    pub sql: String,
    pub params: Vec<Value>,
}

/// Hands out `$n` placeholders and keeps the values behind them.
#[derive(Default)]
struct Binder {
    params: Vec<Value>,
}

impl Binder {
    fn push(&mut self, value: Value) -> String {
        self.params.push(value);
        format!("${}", self.params.len())
    }
}

fn rejected(reason: String) -> StudioError {
    StudioError::Rejected { reason }
}

impl Meta {
    /// `name` as a field the server can be asked about. The name must be a
    /// bare identifier, and a collection with declared fields must declare it.
    /// A schemaless collection declares none, so any bare identifier passes
    /// and the server answers an unknown one with no rows.
    pub fn field<'a>(&self, name: &'a str) -> Result<&'a str, StudioError> {
        let bare = !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        let known = !self.schema.typed_headers
            || self.schema.field(name).is_some()
            || self.schema.id_field.as_deref() == Some(name);
        if bare && known {
            Ok(name)
        } else {
            Err(rejected(format!(
                "`{name}` is not a field of {} that Studio can use in a statement",
                self.collection
            )))
        }
    }

    /// The field that addresses one record. Timeseries and a collection
    /// declared without a primary key have none.
    pub fn key(&self) -> Result<&str, StudioError> {
        self.schema.id_field.as_deref().ok_or_else(|| {
            rejected(format!(
                "{} has no record key, so Studio cannot address one of its records",
                self.collection
            ))
        })
    }

    /// `id` as the value the key column compares against.
    pub fn id_value(&self, id: &str) -> Result<Value, StudioError> {
        let key = self.key()?;
        match self.schema.field(key).map(|f| f.ty) {
            Some(FieldType::Int64) => id
                .parse::<i64>()
                .map(Value::Integer)
                .map_err(|_| rejected(format!("`{id}` is not a number, and {key} holds numbers"))),
            _ => Ok(Value::String(id.to_string())),
        }
    }

    /// The fields a text search reads. A schemaless collection declares none,
    /// so only its key is searched.
    fn text_fields(&self) -> Vec<String> {
        let mut fields: Vec<String> = if self.schema.typed_headers {
            self.schema.filter_fields.clone()
        } else {
            Vec::new()
        };
        if let Some(key) = &self.schema.id_field
            && !fields.contains(key)
        {
            fields.insert(0, key.clone());
        }
        fields
    }
}

/// The key a cell holds, as text. `None` for a cell that is no key.
pub(super) fn id_text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) | Value::Uuid(s) | Value::Ulid(s) => Some(s.clone()),
        Value::Integer(n) => Some(n.to_string()),
        _ => None,
    }
}

/// `%` and `_` in user text match themselves, not any text.
fn escape_like(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn where_clause(
    meta: &Meta,
    query: &RecordQuery,
    now_micros: i64,
    binder: &mut Binder,
) -> Result<String, StudioError> {
    let mut parts: Vec<String> = Vec::new();
    if let Some(filter) = &query.filter {
        let field = meta.field(&filter.field)?;
        let at = binder.push(Value::String(filter.value.clone()));
        parts.push(format!("CAST(\"{field}\" AS TEXT) = {at}"));
    }
    let text = query.text.trim();
    if !text.is_empty() {
        let fields = meta.text_fields();
        if fields.is_empty() {
            return Err(rejected(format!(
                "{} has no field Studio can search",
                meta.collection
            )));
        }
        let at = binder.push(Value::String(format!("%{}%", escape_like(text))));
        let any: Vec<String> = fields
            .iter()
            .map(|f| format!("CAST(\"{f}\" AS TEXT) ILIKE {at} ESCAPE '\\'"))
            .collect();
        parts.push(format!("({})", any.join(" OR ")));
    }
    if meta.profile == Profile::Timeseries
        && let Some(time) = meta.schema.time_key()
    {
        let from = now_micros.saturating_sub(query.range.seconds().saturating_mul(1_000_000));
        let at = binder.push(Value::DateTime(NdbDateTime::from_micros(from)));
        parts.push(format!("\"{}\" >= {at}", meta.field(&time.name)?));
    }
    if let Some(b) = &query.bbox {
        let (geometry, _) = meta.schema.geometry().ok_or_else(|| {
            rejected(format!(
                "{} has no geometry field to filter by area",
                meta.collection
            ))
        })?;
        let field = meta.field(&geometry.name)?;
        let corners =
            [b.min_lon, b.min_lat, b.max_lon, b.max_lat].map(|n| binder.push(Value::Float(n)));
        parts.push(format!(
            "ST_Intersects(\"{field}\", ST_MakeEnvelope({}, {}, {}, {}))",
            corners[0], corners[1], corners[2], corners[3]
        ));
    }
    Ok(if parts.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", parts.join(" AND "))
    })
}

/// A sort clause. The key follows the chosen field so paging stays stable.
fn order_by(meta: &Meta, query: &RecordQuery) -> Result<String, StudioError> {
    let key = meta.schema.id_field.as_deref();
    let (first, desc) = match &query.sort {
        Some((field, desc)) => (Some(meta.field(field)?), *desc),
        None => match (meta.profile, meta.schema.time_key()) {
            (Profile::Timeseries, Some(time)) => (Some(meta.field(&time.name)?), true),
            _ => (key, false),
        },
    };
    let direction = |d: bool| if d { "DESC" } else { "ASC" };
    Ok(match (first, key) {
        (None, _) => String::new(),
        (Some(f), Some(k)) if f != k => {
            format!(" ORDER BY \"{f}\" {}, \"{k}\" ASC", direction(desc))
        }
        (Some(f), _) => format!(" ORDER BY \"{f}\" {}", direction(desc)),
    })
}

/// The page of rows and the count of every row the query matches.
pub(super) fn page_statements(
    meta: &Meta,
    query: &RecordQuery,
    now_micros: i64,
) -> Result<(Statement, Statement), StudioError> {
    let collection = &meta.collection;
    let mut binder = Binder::default();
    let filter = where_clause(meta, query, now_micros, &mut binder)?;
    let count = Statement {
        // A spatial scan cannot aggregate: the server asks for a subquery, and
        // the subquery needs an alias.
        sql: if query.bbox.is_some() {
            format!("SELECT COUNT(*) FROM (SELECT * FROM {collection}{filter}) AS t")
        } else {
            format!("SELECT COUNT(*) FROM {collection}{filter}")
        },
        params: binder.params.clone(),
    };
    let order = order_by(meta, query)?;
    let size = query.page_size.max(1);
    let limit = binder.push(Value::Integer(i64::try_from(size).unwrap_or(i64::MAX)));
    let skip = size.saturating_mul(query.page);
    let offset = binder.push(Value::Integer(i64::try_from(skip).unwrap_or(i64::MAX)));
    let page = Statement {
        sql: format!("SELECT * FROM {collection}{filter}{order} LIMIT {limit} OFFSET {offset}"),
        params: binder.params,
    };
    Ok((page, count))
}

pub(super) fn count_of(result: &QueryResult) -> Result<u64, StudioError> {
    match result.rows.first().and_then(|r| r.first()) {
        Some(Value::Integer(n)) => u64::try_from(*n).map_err(|_| malformed("negative count")),
        _ => Err(malformed("COUNT(*) returned no integer")),
    }
}

fn malformed(reason: &str) -> StudioError {
    StudioError::MalformedRows {
        reason: reason.to_string(),
    }
}

/// FNV-1a over the record's JSON text. Keys print in sorted order, so equal
/// content gives an equal number.
// ponytail: the server exposes no per-record revision in 0.6.2, so this
// content fingerprint stands in for it. It detects any change since the read
// and not the order of changes. Replace it when the server exposes one.
pub(super) fn revision(value: &Value) -> u64 {
    compact(value, &[])
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
        })
}

fn record_of(
    meta: &Meta,
    columns: &[String],
    cells: &[Value],
    ordinal: usize,
) -> Result<Record, StudioError> {
    if cells.len() != columns.len() {
        return Err(malformed("a row's width differs from its header"));
    }
    // A schemaless collection reports a field one document lacks as NULL:
    // absent and null read the same, so null cells drop.
    let keep_null = meta.profile != Profile::Document;
    let map: HashMap<String, Value> = columns
        .iter()
        .cloned()
        .zip(cells.iter().cloned())
        .filter(|(_, v)| keep_null || *v != Value::Null)
        .collect();
    let id = match meta.schema.id_field.as_deref() {
        Some(key) => map.get(key).and_then(id_text).ok_or_else(|| {
            malformed(&format!("a row of {} has no `{key}` key", meta.collection))
        })?,
        // No key: an ordinal names the row for this read only.
        None => format!("row:{ordinal}"),
    };
    let value = Value::Object(map);
    Ok(Record {
        id,
        revision: revision(&value),
        value,
    })
}

/// The rows of `result` as records. `offset` is the first row's position, for
/// a collection without a key.
pub(super) fn records_of(
    meta: &Meta,
    result: &QueryResult,
    offset: usize,
) -> Result<Vec<Record>, StudioError> {
    result
        .rows
        .iter()
        .enumerate()
        .map(|(i, cells)| record_of(meta, &result.columns, cells, offset + i))
        .collect()
}

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

fn unchanged(stored: Option<&Value>, new: &Value) -> bool {
    match (stored, new) {
        (None | Some(Value::Null), Value::Null) => true,
        (Some(s), n) => s == n,
        (None, _) => false,
    }
}

/// The statement that makes `stored` equal `new`, or `None` when they are
/// equal. `UPDATE` merges, so a field `new` lacks is set to NULL. A schemaless
/// collection reads NULL as absent. A key-value entry takes `UPSERT`, because
/// `UPDATE` refuses a bare value.
pub(super) fn update_statement(
    meta: &Meta,
    id: &str,
    stored: &Value,
    new: &Value,
) -> Result<Option<Statement>, StudioError> {
    let key = meta.key()?.to_string();
    let pairs = assignments(meta, new)?;
    same_key(&pairs, &key, id)?;
    let Value::Object(old) = stored else {
        return Err(malformed("a stored record is not an object"));
    };
    let mut set: Vec<(String, Option<Value>)> = pairs
        .iter()
        .filter(|(n, v)| *n != key && !unchanged(old.get(n), v))
        .map(|(n, v)| (n.clone(), Some(v.clone())))
        .collect();
    let mut gone: Vec<&String> = old
        .keys()
        .filter(|n| **n != key && !pairs.iter().any(|(p, _)| p == *n))
        .collect();
    gone.sort();
    for name in gone {
        set.push((meta.field(name)?.to_string(), None));
    }
    if set.is_empty() {
        return Ok(None);
    }
    if meta.profile == Profile::KeyValue {
        let mut all: Vec<(String, Value)> = set
            .into_iter()
            .map(|(n, v)| (n, v.unwrap_or(Value::Null)))
            .collect();
        all.push((key, meta.id_value(id)?));
        all.sort_by(|a, b| a.0.cmp(&b.0));
        return Ok(Some(insert_like("UPSERT", meta, all, false)));
    }
    let mut binder = Binder::default();
    let assigns: Vec<String> = set
        .iter()
        .map(|(n, v)| match v {
            Some(v) => format!("\"{n}\" = {}", binder.push(bindable(v))),
            None => format!("\"{n}\" = NULL"),
        })
        .collect();
    let at = binder.push(meta.id_value(id)?);
    Ok(Some(Statement {
        sql: format!(
            "UPDATE {} SET {} WHERE \"{key}\" = {at}",
            meta.collection,
            assigns.join(", ")
        ),
        params: binder.params,
    }))
}

pub(super) fn delete_statement(meta: &Meta, id: &str) -> Result<Statement, StudioError> {
    let key = meta.key()?;
    Ok(Statement {
        sql: format!("DELETE FROM {} WHERE \"{key}\" = $1", meta.collection),
        params: vec![meta.id_value(id)?],
    })
}

pub(super) fn select_one(meta: &Meta, id: &str) -> Result<Statement, StudioError> {
    let key = meta.key()?;
    Ok(Statement {
        sql: format!(
            "SELECT * FROM {} WHERE \"{key}\" = $1 LIMIT 1",
            meta.collection
        ),
        params: vec![meta.id_value(id)?],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::records::{Bbox, Filter, TimeRange};
    use crate::models::schema::SchemaField;

    fn field(name: &str, ty: FieldType) -> SchemaField {
        SchemaField {
            name: name.into(),
            ty,
            required: false,
            reference: None,
            time_key: false,
        }
    }

    fn strict() -> Meta {
        let mut schema = CollectionSchema::new("c");
        schema.id_field = Some("id".into());
        schema.typed_headers = true;
        schema.fields = vec![
            field("id", FieldType::String { max: None }),
            field("name", FieldType::String { max: None }),
            field("qty", FieldType::Int64),
        ];
        schema.filter_fields = vec!["id".into(), "name".into(), "qty".into()];
        Meta {
            collection: "c".into(),
            profile: Profile::DocumentStrict,
            schema,
        }
    }

    fn schemaless() -> Meta {
        let mut schema = CollectionSchema::new("d");
        schema.id_field = Some("id".into());
        Meta {
            collection: "d".into(),
            profile: Profile::Document,
            schema,
        }
    }

    fn obj(fields: &[(&str, Value)]) -> Value {
        Value::Object(
            fields
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
        )
    }

    fn s(text: &str) -> Value {
        Value::String(text.into())
    }

    #[test]
    fn a_field_must_be_bare_and_declared() {
        let m = strict();
        assert_eq!(m.field("name").ok(), Some("name"));
        assert!(m.field("nope").is_err());
        assert!(m.field("name\" OR 1=1 --").is_err());
        // a schemaless collection declares nothing: any bare name passes
        let d = schemaless();
        assert!(d.field("anything_9").is_ok());
        assert!(d.field("a b").is_err());
        assert!(d.field("a;b").is_err());
    }

    #[test]
    fn the_default_page_orders_by_key_and_binds_limit_and_offset() {
        let mut q = RecordQuery::new(10);
        q.page = 2;
        let (page, count) = page_statements(&strict(), &q, 0).expect("builds");
        assert_eq!(
            page.sql,
            "SELECT * FROM c ORDER BY \"id\" ASC LIMIT $1 OFFSET $2"
        );
        assert_eq!(page.params, [Value::Integer(10), Value::Integer(20)]);
        assert_eq!(count.sql, "SELECT COUNT(*) FROM c");
        assert!(count.params.is_empty());
    }

    #[test]
    fn filter_text_and_sort_bind_values_and_check_names() {
        let q = RecordQuery {
            filter: Some(Filter {
                field: "qty".into(),
                value: "3".into(),
            }),
            text: " a%b ".into(),
            sort: Some(("name".into(), true)),
            ..RecordQuery::new(5)
        };
        let (page, count) = page_statements(&strict(), &q, 0).expect("builds");
        assert_eq!(
            page.sql,
            "SELECT * FROM c WHERE CAST(\"qty\" AS TEXT) = $1 AND \
             (CAST(\"id\" AS TEXT) ILIKE $2 ESCAPE '\\' OR CAST(\"name\" AS TEXT) ILIKE $2 ESCAPE '\\' \
             OR CAST(\"qty\" AS TEXT) ILIKE $2 ESCAPE '\\') \
             ORDER BY \"name\" DESC, \"id\" ASC LIMIT $3 OFFSET $4"
        );
        assert_eq!(page.params[0], s("3"));
        assert_eq!(page.params[1], s("%a\\%b%"));
        assert_eq!(count.params.len(), 2);

        for bad in [
            RecordQuery {
                filter: Some(Filter {
                    field: "x\" OR 1=1".into(),
                    value: "v".into(),
                }),
                ..RecordQuery::new(5)
            },
            RecordQuery {
                sort: Some(("nope".into(), false)),
                ..RecordQuery::new(5)
            },
        ] {
            assert!(page_statements(&strict(), &bad, 0).is_err());
        }
    }

    #[test]
    fn a_timeseries_reads_newest_first_inside_its_range() {
        let mut schema = CollectionSchema::new("t");
        schema.typed_headers = true;
        let mut ts = field("ts", FieldType::Timestamp);
        ts.time_key = true;
        schema.fields = vec![ts, field("cpu", FieldType::Float64)];
        let meta = Meta {
            collection: "t".into(),
            profile: Profile::Timeseries,
            schema,
        };
        let q = RecordQuery {
            range: TimeRange::Last24Hours,
            ..RecordQuery::new(5)
        };
        let (page, _) = page_statements(&meta, &q, 100_000_000_000).expect("builds");
        assert_eq!(
            page.sql,
            "SELECT * FROM t WHERE \"ts\" >= $1 ORDER BY \"ts\" DESC LIMIT $2 OFFSET $3"
        );
        assert_eq!(
            page.params[0],
            Value::DateTime(NdbDateTime::from_micros(100_000_000_000 - 86_400_000_000))
        );
    }

    #[test]
    fn a_bbox_needs_a_geometry_field_and_counts_through_a_subquery() {
        let mut schema = CollectionSchema::new("p");
        schema.typed_headers = true;
        schema.fields = vec![field("geom", FieldType::Geometry { srid: 4326 })];
        let meta = Meta {
            collection: "p".into(),
            profile: Profile::Spatial,
            schema,
        };
        let q = RecordQuery {
            bbox: Bbox::parse("-74, 40, -73, 41").ok(),
            ..RecordQuery::new(5)
        };
        let (page, count) = page_statements(&meta, &q, 0).expect("builds");
        assert!(
            page.sql
                .contains("ST_Intersects(\"geom\", ST_MakeEnvelope($1, $2, $3, $4))")
        );
        assert!(
            count
                .sql
                .starts_with("SELECT COUNT(*) FROM (SELECT * FROM p WHERE")
        );
        assert!(page_statements(&strict(), &q, 0).is_err());
    }

    #[test]
    fn rows_decode_into_records_keyed_and_fingerprinted() {
        let result = QueryResult {
            columns: vec!["id".into(), "n".into(), "name".into()],
            rows: vec![
                vec![s("a"), Value::Integer(2), s("alice")],
                vec![s("b"), Value::Null, s("bob")],
            ],
            rows_affected: 2,
            command: None,
        };
        let rows = records_of(&schemaless(), &result, 0).expect("decodes");
        assert_eq!(rows[0].id, "a");
        assert_eq!(
            rows[1].get("n"),
            None,
            "a schemaless NULL is an absent field"
        );
        let strict_rows = records_of(&strict(), &result, 0).expect("decodes");
        assert_eq!(strict_rows[1].get("n"), Some(&Value::Null));
        assert_ne!(rows[0].revision, rows[1].revision);
        assert_eq!(
            rows[0].revision,
            revision(&obj(&[
                ("id", s("a")),
                ("n", Value::Integer(2)),
                ("name", s("alice"))
            ]))
        );
    }

    #[test]
    fn a_collection_without_a_key_numbers_its_rows_and_refuses_key_work() {
        let meta = Meta {
            collection: "t".into(),
            profile: Profile::Timeseries,
            schema: CollectionSchema::new("t"),
        };
        let result = QueryResult {
            columns: vec!["cpu".into()],
            rows: vec![vec![Value::Float(1.0)], vec![Value::Float(2.0)]],
            rows_affected: 0,
            command: None,
        };
        let rows = records_of(&meta, &result, 10).expect("decodes");
        assert_eq!(
            (rows[0].id.as_str(), rows[1].id.as_str()),
            ("row:10", "row:11")
        );
        assert!(matches!(meta.key(), Err(StudioError::Rejected { .. })));
        assert!(delete_statement(&meta, "x").is_err());
    }

    #[test]
    fn a_row_without_its_key_or_width_is_malformed() {
        let no_key = QueryResult {
            columns: vec!["name".into()],
            rows: vec![vec![s("x")]],
            rows_affected: 0,
            command: None,
        };
        assert!(matches!(
            records_of(&strict(), &no_key, 0),
            Err(StudioError::MalformedRows { .. })
        ));
        let narrow = QueryResult {
            columns: vec!["id".into(), "name".into()],
            rows: vec![vec![s("x")]],
            rows_affected: 0,
            command: None,
        };
        assert!(records_of(&strict(), &narrow, 0).is_err());
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
    fn an_update_sets_changed_fields_and_nulls_removed_ones() {
        let stored = obj(&[
            ("id", s("a")),
            ("name", s("alice")),
            ("n", Value::Integer(1)),
        ]);
        let new = obj(&[("id", s("a")), ("name", s("alice2"))]);
        let st = update_statement(&schemaless(), "a", &stored, &new)
            .expect("builds")
            .expect("changes");
        assert_eq!(
            st.sql,
            "UPDATE d SET \"name\" = $1, \"n\" = NULL WHERE \"id\" = $2"
        );
        assert_eq!(st.params, [s("alice2"), s("a")]);
        assert_eq!(
            update_statement(&schemaless(), "a", &stored, &stored).expect("builds"),
            None
        );
        // an explicit null over an absent field is no change
        let nulled = obj(&[
            ("id", s("a")),
            ("name", s("alice")),
            ("n", Value::Integer(1)),
            ("x", Value::Null),
        ]);
        assert_eq!(
            update_statement(&schemaless(), "a", &stored, &nulled).expect("builds"),
            None
        );
        let renamed = obj(&[("id", s("b"))]);
        assert!(update_statement(&schemaless(), "a", &stored, &renamed).is_err());
    }

    #[test]
    fn a_key_value_update_upserts() {
        let mut schema = CollectionSchema::new("kv");
        schema.id_field = Some("key".into());
        let meta = Meta {
            collection: "kv".into(),
            profile: Profile::KeyValue,
            schema,
        };
        let stored = obj(&[("key", s("k")), ("value", s("v"))]);
        let new = obj(&[("key", s("k")), ("value", s("w"))]);
        let st = update_statement(&meta, "k", &stored, &new)
            .expect("builds")
            .expect("changes");
        assert_eq!(
            st.sql,
            "UPSERT INTO kv (\"key\", \"value\") VALUES ($1, $2)"
        );
        assert_eq!(st.params, [s("k"), s("w")]);
    }

    #[test]
    fn a_numeric_key_binds_as_a_number() {
        let mut meta = strict();
        meta.schema.fields[0] = field("id", FieldType::Int64);
        assert_eq!(meta.id_value("20").ok(), Some(Value::Integer(20)));
        assert!(meta.id_value("x").is_err());
        assert_eq!(
            delete_statement(&meta, "20").expect("builds").params,
            [Value::Integer(20)]
        );
    }

    #[test]
    fn the_revision_follows_content_only() {
        let a = obj(&[("x", Value::Integer(1)), ("y", s("v"))]);
        let b = obj(&[("y", s("v")), ("x", Value::Integer(1))]);
        assert_eq!(revision(&a), revision(&b));
        assert_ne!(
            revision(&a),
            revision(&obj(&[("x", Value::Integer(2)), ("y", s("v"))]))
        );
    }
}
