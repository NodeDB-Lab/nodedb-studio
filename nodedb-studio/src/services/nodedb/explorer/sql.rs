//! Record reads built from validated parts, and rows decoded into records.
//! Writes are in `explorer::write`.
//!
//! Values travel as `$n` parameters. Identifiers (collection, field names)
//! cannot be bound, so each one passes `Meta::field` or `ident` before it
//! reaches the text, a name the server reports included. A field name always
//! reaches the text inside double quotes. No function here touches the
//! network.

use std::collections::HashMap;

use nodedb_client::QueryResult;
use nodedb_types::{NdbDateTime, Value};

use crate::models::collection::Profile;
use crate::models::records::{Record, RecordQuery};
use crate::models::schema::{CollectionSchema, FieldType};
use crate::services::error::StudioError;

/// What a write or read needs to know about one collection: its storage
/// profile and its declared shape.
pub(in crate::services::nodedb) struct Meta {
    pub collection: String,
    pub profile: Profile,
    pub schema: CollectionSchema,
}

/// A statement with its bound values, in `$n` order.
#[derive(Debug, Clone, PartialEq)]
pub(in crate::services::nodedb) struct Statement {
    pub sql: String,
    pub params: Vec<Value>,
}

/// Hands out `$n` placeholders and keeps the values behind them.
#[derive(Default)]
pub(in crate::services::nodedb) struct Binder {
    pub params: Vec<Value>,
}

impl Binder {
    pub fn push(&mut self, value: Value) -> String {
        self.params.push(value);
        format!("${}", self.params.len())
    }
}

pub(in crate::services::nodedb) fn rejected(reason: String) -> StudioError {
    StudioError::Rejected { reason }
}

impl Meta {
    /// `name` as a field the server can be asked about, for the caller to
    /// write as `"name"`. Checked live on 0.6.2: a quoted name with a dash, a
    /// space, a `'`, `$1`, `--` or non-ASCII letters works in INSERT, UPDATE
    /// SET and WHERE, and the server refuses a name holding `"`. So a name
    /// passes unless it is empty or holds `"` or a control character, which
    /// keeps it inside its quotes. A collection with declared fields must
    /// declare it. A schemaless collection declares none, so any such name
    /// passes and the server answers an unknown one with no rows.
    pub fn field<'a>(&self, name: &'a str) -> Result<&'a str, StudioError> {
        let why = if name.is_empty() {
            Some("an empty field name cannot be used in a statement".to_string())
        } else if name.contains('"') {
            Some(format!(
                "`{name}` holds a double quote, and NodeDB 0.6.2 refuses such a field name \
                 in a statement: Studio cannot write or filter by it"
            ))
        } else if name.chars().any(char::is_control) {
            Some(format!(
                "{name:?} holds a control character, which Studio does not put in a statement"
            ))
        } else if self.schema.typed_headers
            && self.schema.field(name).is_none()
            && self.schema.id_field.as_deref() != Some(name)
        {
            Some(format!("`{name}` is not a field of {}", self.collection))
        } else {
            None
        };
        match why {
            None => Ok(name),
            Some(reason) => Err(rejected(reason)),
        }
    }

    /// The field that addresses one record, checked like any field name.
    /// Timeseries has none.
    pub fn key(&self) -> Result<&str, StudioError> {
        let key = self.schema.id_field.as_deref().ok_or_else(|| {
            rejected(format!(
                "{} has no record key, so Studio cannot address one of its records",
                self.collection
            ))
        })?;
        self.field(key)
    }

    /// `id` as the value the key column compares against.
    pub fn id_value(&self, id: &str) -> Result<Value, StudioError> {
        let key = self.key()?;
        match self.schema.field(key).map(|f| f.ty) {
            Some(FieldType::Int64) => id
                .parse::<i64>()
                .map(Value::Integer)
                .map_err(|_| rejected(format!("`{id}` is not a number, and {key} holds numbers"))),
            Some(
                FieldType::String { .. }
                | FieldType::Float64
                | FieldType::Decimal { .. }
                | FieldType::Bool
                | FieldType::Timestamp
                | FieldType::Datetime
                | FieldType::Uuid
                | FieldType::Object
                | FieldType::Array
                | FieldType::Json
                | FieldType::Vector { .. }
                | FieldType::Geometry { .. }
                | FieldType::Bytes,
            )
            | None => Ok(Value::String(id.to_string())),
        }
    }

    /// The fields a text search reads, each name checked. A schemaless
    /// collection declares no fields, and 0.6.2 has no statement that matches
    /// text anywhere in a document: `document` reads as NULL in a filter and
    /// `text_match` needs a full-text index (checked live).
    fn text_fields(&self) -> Result<Vec<&str>, StudioError> {
        if !self.schema.typed_headers {
            return Err(rejected(format!(
                "text search on schemaless collection {} is not available on NodeDB 0.6.2: \
                 filter by a field instead",
                self.collection
            )));
        }
        let mut fields: Vec<&str> = Vec::new();
        if self.schema.id_field.is_some() {
            fields.push(self.key()?);
        }
        for f in &self.schema.filter_fields {
            let name = self.field(f)?;
            if !fields.contains(&name) {
                fields.push(name);
            }
        }
        Ok(fields)
    }
}

/// The key a cell holds, as text. `None` for a cell that is no key.
pub(in crate::services::nodedb) fn id_text(value: &Value) -> Option<String> {
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
        let fields = meta.text_fields()?;
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
    let key = match meta.schema.id_field {
        Some(_) => Some(meta.key()?),
        None => None,
    };
    let (first, desc) = match &query.sort {
        Some((field, desc)) => (Some(meta.field(field)?), *desc),
        None => match (meta.profile, meta.schema.time_key()) {
            (Profile::Timeseries, Some(time)) => (Some(meta.field(&time.name)?), true),
            (Profile::Timeseries, None)
            | (
                Profile::Document
                | Profile::DocumentStrict
                | Profile::KeyValue
                | Profile::Columnar
                | Profile::Spatial
                | Profile::Array
                | Profile::Vector,
                _,
            ) => (key, false),
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
pub(in crate::services::nodedb) fn page_statements(
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

pub(in crate::services::nodedb) fn count_of(result: &QueryResult) -> Result<u64, StudioError> {
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

/// `value` as text that changes whenever the value does: object keys in
/// sorted order, every other value through its derived `Debug`, which prints
/// every byte, every vector component and a float's exact value, NaN and the
/// infinities included. The display text cannot serve: it prints bytes as
/// `<N bytes>`, a vector as its first components and NaN as `null`.
fn canonical(value: &Value, out: &mut String) {
    let items = |tag: &str, items: &[Value], out: &mut String| {
        out.push_str(tag);
        for item in items {
            canonical(item, out);
            out.push(',');
        }
        out.push(']');
    };
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by(|a, b| a.0.cmp(b.0));
            out.push('{');
            for (k, v) in entries {
                out.push_str(&format!("{k:?}:"));
                canonical(v, out);
                out.push(',');
            }
            out.push('}');
        }
        Value::Array(list) => items("A[", list, out),
        Value::Set(list) => items("S[", list, out),
        other => out.push_str(&format!("{other:?}")),
    }
}

/// FNV-1a over the record's canonical text, so equal content gives an equal
/// number.
// ponytail: the server exposes no per-record revision in 0.6.2, so this
// content fingerprint stands in for it. It detects any change since the read
// and not the order of changes. Replace it when the server exposes one.
pub(in crate::services::nodedb) fn revision(value: &Value) -> u64 {
    let mut text = String::new();
    canonical(value, &mut text);
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// One row as a record value. A schemaless collection reports a field one
/// document lacks as NULL: absent and null read the same, so null cells drop.
fn row_value(meta: &Meta, columns: &[String], cells: &[Value]) -> Result<Value, StudioError> {
    if cells.len() != columns.len() {
        return Err(malformed("a row's width differs from its header"));
    }
    let keep_null = meta.profile != Profile::Document;
    Ok(Value::Object(
        columns
            .iter()
            .cloned()
            .zip(cells.iter().cloned())
            .filter(|(_, v)| keep_null || *v != Value::Null)
            .collect(),
    ))
}

/// The time a row of a collection without a key carries, in microseconds.
fn row_time(meta: &Meta, value: &Value) -> Option<i64> {
    let name = &meta.schema.time_key()?.name;
    match value {
        Value::Object(map) => match map.get(name)? {
            Value::DateTime(t) | Value::NaiveDateTime(t) => Some(t.micros),
            _ => None,
        },
        _ => None,
    }
}

/// The rows of `result` as records.
///
/// A collection without a key (timeseries) names a row `t<micros>.<hex>`:
/// its time and its content fingerprint. The n-th repeat of an identical row
/// in one result adds `.<n>`. A new row renames no other, and
/// `select_one` reads the id back.
pub(in crate::services::nodedb) fn records_of(
    meta: &Meta,
    result: &QueryResult,
) -> Result<Vec<Record>, StudioError> {
    let mut repeats: HashMap<String, usize> = HashMap::new();
    result
        .rows
        .iter()
        .map(|cells| {
            let value = row_value(meta, &result.columns, cells)?;
            let revision = revision(&value);
            let id = match meta.schema.id_field.as_deref() {
                Some(key) => match &value {
                    Value::Object(map) => map.get(key).and_then(id_text),
                    _ => None,
                }
                .ok_or_else(|| {
                    malformed(&format!("a row of {} has no `{key}` key", meta.collection))
                })?,
                None => {
                    let time = row_time(meta, &value)
                        .map(|t| t.to_string())
                        .unwrap_or_default();
                    let base = format!("t{time}.{revision:016x}");
                    let n = repeats.entry(base.clone()).or_insert(0);
                    let id = if *n == 0 { base } else { format!("{base}.{n}") };
                    *n += 1;
                    id
                }
            };
            Ok(Record {
                id,
                revision,
                value,
            })
        })
        .collect()
}

/// The statement that reads record `id`: by its key, or for a collection
/// without one, every row at the time the id names. The caller picks the row
/// whose id `records_of` gives as `id`.
pub(in crate::services::nodedb) fn select_one(
    meta: &Meta,
    id: &str,
) -> Result<Statement, StudioError> {
    if meta.schema.id_field.is_some() {
        let key = meta.key()?;
        return Ok(Statement {
            sql: format!(
                "SELECT * FROM {} WHERE \"{key}\" = $1 LIMIT 1",
                meta.collection
            ),
            params: vec![meta.id_value(id)?],
        });
    }
    let time = meta.schema.time_key().ok_or_else(|| {
        rejected(format!(
            "{} has no record key and no time key, so Studio cannot address one of its records",
            meta.collection
        ))
    })?;
    let at = id
        .strip_prefix('t')
        .and_then(|rest| rest.split('.').next())
        .and_then(|t| t.parse::<i64>().ok())
        .ok_or_else(|| rejected(format!("`{id}` does not name a row of {}", meta.collection)))?;
    Ok(Statement {
        sql: format!(
            "SELECT * FROM {} WHERE \"{}\" = $1",
            meta.collection,
            meta.field(&time.name)?
        ),
        params: vec![Value::DateTime(NdbDateTime::from_micros(at))],
    })
}

#[cfg(test)]
pub(in crate::services::nodedb) mod tests {
    use super::*;
    use crate::models::records::{Bbox, Filter, TimeRange};
    use crate::models::schema::SchemaField;

    pub(in crate::services::nodedb) fn field(name: &str, ty: FieldType) -> SchemaField {
        SchemaField {
            name: name.into(),
            ty,
            required: false,
            reference: None,
            time_key: false,
        }
    }

    pub(in crate::services::nodedb) fn strict() -> Meta {
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

    pub(in crate::services::nodedb) fn schemaless() -> Meta {
        let mut schema = CollectionSchema::new("d");
        schema.id_field = Some("id".into());
        Meta {
            collection: "d".into(),
            profile: Profile::Document,
            schema,
        }
    }

    pub(in crate::services::nodedb) fn timeseries() -> Meta {
        let mut schema = CollectionSchema::new("t");
        schema.typed_headers = true;
        let mut ts = field("ts", FieldType::Timestamp);
        ts.time_key = true;
        schema.fields = vec![ts, field("cpu", FieldType::Float64)];
        Meta {
            collection: "t".into(),
            profile: Profile::Timeseries,
            schema,
        }
    }

    pub(in crate::services::nodedb) fn obj(fields: &[(&str, Value)]) -> Value {
        Value::Object(
            fields
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
        )
    }

    pub(in crate::services::nodedb) fn s(text: &str) -> Value {
        Value::String(text.into())
    }

    fn result(columns: &[&str], rows: Vec<Vec<Value>>) -> QueryResult {
        QueryResult {
            columns: columns.iter().map(|c| c.to_string()).collect(),
            rows,
            rows_affected: 0,
            command: None,
        }
    }

    #[test]
    fn a_field_must_stay_inside_its_quotes_and_be_declared() {
        let m = strict();
        assert_eq!(m.field("name").ok(), Some("name"));
        assert!(m.field("nope").is_err());
        assert!(m.field("name\" OR 1=1 --").is_err());
        // a schemaless collection declares nothing: any quotable name passes
        let d = schemaless();
        for ok in [
            "anything_9",
            "e-mail",
            "a b",
            "a;b",
            "o'k",
            "名前",
            "a$1",
            "x--y",
        ] {
            assert_eq!(d.field(ok).ok(), Some(ok), "{ok}");
        }
        for bad in ["", "a\"b", "a\0b", "a\nb", "a\u{7f}"] {
            assert!(
                matches!(d.field(bad), Err(StudioError::Rejected { .. })),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn a_name_the_server_reports_passes_the_same_check() {
        let reason = |e: StudioError| match e {
            StudioError::Rejected { reason } => reason,
            other => panic!("expected a refusal, got {other:?}"),
        };
        let mut m = strict();
        m.schema.id_field = Some("i\"d".into());
        m.schema.fields[0].name = "i\"d".into();
        let e = reason(page_statements(&m, &RecordQuery::new(5), 0).expect_err("key"));
        assert!(e.contains("i\"d"), "{e}");
        assert!(select_one(&m, "x").is_err());
        assert!(m.id_value("x").is_err());

        let mut m = strict();
        m.schema.fields[1].name = "na\"me".into();
        m.schema.filter_fields[1] = "na\"me".into();
        let q = RecordQuery {
            text: "a".into(),
            ..RecordQuery::new(5)
        };
        let e = reason(page_statements(&m, &q, 0).expect_err("searched field"));
        assert!(e.contains("na\"me"), "{e}");
    }

    #[test]
    fn text_search_on_a_schemaless_collection_is_refused_not_empty() {
        let q = RecordQuery {
            text: "alice".into(),
            ..RecordQuery::new(5)
        };
        match page_statements(&schemaless(), &q, 0) {
            Err(StudioError::Rejected { reason }) => assert!(reason.contains("0.6.2"), "{reason}"),
            other => panic!("expected a refusal, got {other:?}"),
        }
        assert!(page_statements(&schemaless(), &RecordQuery::new(5), 0).is_ok());
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
        let meta = timeseries();
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
        schema.fields = vec![field("geom", FieldType::Geometry { srid: None })];
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
        let rows = records_of(&schemaless(), &result).expect("decodes");
        assert_eq!(rows[0].id, "a");
        assert_eq!(
            rows[1].get("n"),
            None,
            "a schemaless NULL is an absent field"
        );
        let strict_rows = records_of(&strict(), &result).expect("decodes");
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
    fn a_row_without_its_key_or_width_is_malformed() {
        let no_key = QueryResult {
            columns: vec!["name".into()],
            rows: vec![vec![s("x")]],
            rows_affected: 0,
            command: None,
        };
        assert!(matches!(
            records_of(&strict(), &no_key),
            Err(StudioError::MalformedRows { .. })
        ));
        let narrow = QueryResult {
            columns: vec!["id".into(), "name".into()],
            rows: vec![vec![s("x")]],
            rows_affected: 0,
            command: None,
        };
        assert!(records_of(&strict(), &narrow).is_err());
    }

    #[test]
    fn a_timeseries_row_is_named_by_its_time_and_content() {
        let meta = timeseries();
        let at = |micros: i64| Value::NaiveDateTime(NdbDateTime::from_micros(micros));
        let row = |t: i64, cpu: f64| vec![Value::Float(cpu), at(t)];
        let first = records_of(
            &meta,
            &result(
                &["cpu", "ts"],
                vec![row(5_000, 1.0), row(5_000, 1.0), row(5_000, 2.0)],
            ),
        )
        .expect("decodes");
        assert!(first[0].id.starts_with("t5000."), "{}", first[0].id);
        assert_eq!(
            first[1].id,
            format!("{}.1", first[0].id),
            "a repeat gets a tiebreak"
        );
        assert_ne!(first[2].id, first[0].id);
        // a newer point read first renames none of the others
        let later = records_of(
            &meta,
            &result(
                &["cpu", "ts"],
                vec![
                    row(9_000, 3.0),
                    row(5_000, 1.0),
                    row(5_000, 1.0),
                    row(5_000, 2.0),
                ],
            ),
        )
        .expect("decodes");
        let ids = |r: &[Record]| r.iter().map(|x| x.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&later[1..]), ids(&first));

        let read = select_one(&meta, &first[2].id).expect("reads by time");
        assert_eq!(read.sql, "SELECT * FROM t WHERE \"ts\" = $1");
        assert_eq!(
            read.params,
            [Value::DateTime(NdbDateTime::from_micros(5_000))]
        );
        assert!(matches!(meta.key(), Err(StudioError::Rejected { .. })));
        for bad in ["row:0", "t", "tx.1", ""] {
            assert!(select_one(&meta, bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_numeric_key_binds_as_a_number() {
        let mut meta = strict();
        meta.schema.fields[0] = field("id", FieldType::Int64);
        assert_eq!(meta.id_value("20").ok(), Some(Value::Integer(20)));
        assert!(meta.id_value("x").is_err());
        assert_eq!(
            select_one(&meta, "20").expect("builds").params,
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

    #[test]
    fn the_revision_sees_what_the_display_shortens() {
        let one = |v: Value| revision(&obj(&[("v", v)]));
        let vector = |last: f32| Value::Vector(vec![0.5, 0.25, 0.125, last].into());
        assert_ne!(
            one(vector(1.0)),
            one(vector(2.0)),
            "a change after the third component"
        );
        assert_ne!(
            one(Value::Bytes(vec![1, 2])),
            one(Value::Bytes(vec![1, 3])),
            "bytes of one length"
        );
        let floats = [f64::NAN, f64::INFINITY, f64::NEG_INFINITY];
        for (i, x) in floats.iter().enumerate() {
            assert_ne!(one(Value::Float(*x)), one(Value::Null), "{x}");
            for y in &floats[i + 1..] {
                assert_ne!(one(Value::Float(*x)), one(Value::Float(*y)), "{x} {y}");
            }
        }
        assert_ne!(one(Value::Float(1.0)), one(s("1.0")));
        assert_ne!(
            one(Value::Array(vec![Value::Integer(1)])),
            one(Value::Set(vec![Value::Integer(1)]))
        );
    }
}
