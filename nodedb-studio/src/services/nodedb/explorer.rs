//! Catalog, record, and array access.
//!
//! Live today: `catalog` and `collection_schema`. Every other method is a
//! `NotConnected` stub.

use async_trait::async_trait;
use nodedb_client::NodeDbError;
use nodedb_types::Value;

use crate::models::array_data::{ArraySchema, SliceRequest, SliceResult};
use crate::models::collection::{Attached, Collection, Profile};
use crate::models::graph_records::{EdgePage, EdgeQuery};
use crate::models::records::{CollectionAccess, Record, RecordPage, RecordQuery};
use crate::models::schema::{
    CellFormat, CollectionSchema, ColumnSpec, FieldType, IndexDef, IndexKind, SchemaField,
    TextIndex, VectorField,
};
use crate::services::decode::{Table, decode_rows};
use crate::services::error::StudioError;
use crate::services::explorer_data::ExplorerData;
use crate::services::nodedb::NodeDbConnectionService;

/// `SHOW COLLECTIONS` reports an array with this `partition_strategy`.
const ARRAY_STRATEGY: &str = "array";

/// `name` as a bare SQL identifier, or the server's not-found error. A name
/// outside `[A-Za-z0-9_]` never reaches the SQL text.
fn ident(name: &str) -> Result<&str, StudioError> {
    let ok = !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if ok {
        Ok(name)
    } else {
        Err(StudioError::NotFound(NodeDbError::collection_not_found(
            name,
        )))
    }
}

fn malformed(reason: String) -> StudioError {
    StudioError::MalformedRows { reason }
}

/// The profile a `__storage` value names. `None` for a value Studio has no
/// profile for (the server also has a `vector` storage).
fn profile_of_storage(storage: &str) -> Option<Profile> {
    match storage {
        "document_schemaless" => Some(Profile::Document),
        "document_strict" => Some(Profile::DocumentStrict),
        "kv" => Some(Profile::KeyValue),
        "columnar" => Some(Profile::Columnar),
        "timeseries" => Some(Profile::Timeseries),
        "spatial" => Some(Profile::Spatial),
        _ => None,
    }
}

/// The `__storage` row of a `DESCRIBE`. A document collection may omit it.
fn profile_of_describe(collection: &str, describe: &Table) -> Result<Profile, StudioError> {
    let rows = decode_rows(describe, &["field", "type"], |r| {
        Ok((r.field("field")?.to_string(), r.field("type")?.to_string()))
    })?;
    match rows.iter().find(|(f, _)| f == "__storage") {
        None => Ok(Profile::Document),
        Some((_, storage)) => profile_of_storage(storage).ok_or_else(|| {
            malformed(format!(
                "collection {collection} has storage `{storage}`, which Studio has no profile for"
            ))
        }),
    }
}

/// One `SHOW INDEXES` row.
#[derive(Debug, Clone, PartialEq, Eq)]
struct IndexRow {
    name: String,
    kind: String,
    collection: String,
    fields: Vec<String>,
}

fn decode_indexes(table: &Table) -> Result<Vec<IndexRow>, StudioError> {
    decode_rows(
        table,
        &["index_name", "type", "collection", "fields"],
        |r| {
            Ok(IndexRow {
                name: r.field("index_name")?.to_string(),
                kind: r.field("type")?.to_string(),
                collection: r.field("collection")?.to_string(),
                // btree fields read `$.name`: the document path, not the field.
                fields: r
                    .field("fields")?
                    .split(',')
                    .map(|f| f.trim().trim_start_matches("$.").to_string())
                    .filter(|f| !f.is_empty())
                    .collect(),
            })
        },
    )
}

/// The capabilities a collection's indexes attach.
fn attached_of(indexes: &[IndexRow], collection: &str) -> Vec<Attached> {
    let has = |kind: &str| {
        indexes
            .iter()
            .any(|i| i.collection == collection && i.kind == kind)
    };
    // Graph stays out: the server has no catalog statement that says a
    // collection carries edges (`GRAPH` statements name a collection, nothing lists them).
    [
        ("fulltext", Attached::FullText),
        ("vector", Attached::Vector),
    ]
    .into_iter()
    .filter(|(kind, _)| has(kind))
    .map(|(_, a)| a)
    .collect()
}

/// The declared type of a `DESCRIBE` type string such as `TEXT NOT NULL`,
/// `VECTOR(3)` or `TIMESTAMP TIME_KEY`. A type Studio does not know reads as
/// `Json` (any value), and the geometry SRID, which the server does not
/// report, reads as 4326.
fn parse_type(raw: &str) -> FieldType {
    let head = raw.split_whitespace().next().unwrap_or("");
    let (name, args) = match head.split_once('(') {
        Some((n, rest)) => (n, rest.trim_end_matches(')')),
        None => (head, ""),
    };
    let nums: Vec<u32> = args
        .split(',')
        .filter_map(|a| a.trim().parse().ok())
        .collect();
    match name.to_ascii_uppercase().as_str() {
        "TEXT" | "STRING" => FieldType::String { max: None },
        "VARCHAR" | "CHAR" => FieldType::String {
            max: nums.first().copied(),
        },
        "INT" | "INTEGER" | "BIGINT" | "SMALLINT" | "INT64" | "INT4" | "INT8" => FieldType::Int64,
        "FLOAT" | "DOUBLE" | "REAL" | "FLOAT64" | "FLOAT8" => FieldType::Float64,
        "DECIMAL" | "NUMERIC" => FieldType::Decimal {
            precision: nums
                .first()
                .and_then(|n| u8::try_from(*n).ok())
                .unwrap_or(38),
            scale: nums.get(1).and_then(|n| u8::try_from(*n).ok()).unwrap_or(0),
        },
        "BOOL" | "BOOLEAN" => FieldType::Bool,
        "TIMESTAMP" | "TIMESTAMPTZ" => FieldType::Timestamp,
        "DATETIME" => FieldType::Datetime,
        "UUID" => FieldType::Uuid,
        "OBJECT" => FieldType::Object,
        "ARRAY" => FieldType::Array,
        "BYTES" | "BYTEA" | "BLOB" => FieldType::Bytes,
        "VECTOR" => FieldType::Vector {
            dimensions: nums.first().copied().unwrap_or(0),
        },
        "GEOMETRY" => FieldType::Geometry { srid: 4326 },
        _ => FieldType::Json,
    }
}

fn cell_format(ty: FieldType, is_id: bool) -> CellFormat {
    if is_id {
        return CellFormat::Id;
    }
    match ty {
        FieldType::Int64 | FieldType::Float64 | FieldType::Decimal { .. } => CellFormat::Number,
        FieldType::Timestamp | FieldType::Datetime => CellFormat::Timestamp,
        FieldType::Geometry { .. } => CellFormat::Geometry,
        FieldType::Vector { .. } => CellFormat::Embedding,
        FieldType::Bytes => CellFormat::Bytes,
        FieldType::Object | FieldType::Array | FieldType::Json => CellFormat::Preview,
        FieldType::String { .. } | FieldType::Bool | FieldType::Uuid => CellFormat::Text,
    }
}

fn index_kind(kind: &str) -> IndexKind {
    match kind {
        "vector" => IndexKind::Vector,
        "fulltext" => IndexKind::FullText,
        "spatial" => IndexKind::Spatial,
        // btree, sparse, sorted
        _ => IndexKind::Secondary,
    }
}

/// Build the schema from a `DESCRIBE` and the `SHOW INDEXES ON` of the same
/// collection.
///
/// The server does not report: references, the default sort, a page size,
/// the graph binding, the analyzer of a full-text field, index build state or
/// coverage, a vector's model, revision and normalization, or a geometry's
/// SRID. Each reads as its empty value (or 4326 for the SRID).
fn schema_of(
    collection: &str,
    profile: Profile,
    describe: &Table,
    indexes: &[IndexRow],
) -> Result<CollectionSchema, StudioError> {
    let rows = decode_rows(
        describe,
        &["field", "type", "nullable", "primary_key"],
        |r| {
            Ok((
                r.field("field")?.to_string(),
                r.field("type")?.to_string(),
                r.field("nullable")? == "true",
                r.field("primary_key")? == "true",
            ))
        },
    )?;
    // A schemaless collection reports only `id` and `document`: no declared fields.
    let declared = profile != Profile::Document;
    let synthetic_id = matches!(profile, Profile::KeyValue | Profile::Timeseries);
    let mut schema = CollectionSchema::new(collection);
    for (name, raw, nullable, pk) in &rows {
        if name.starts_with("__") || !declared {
            if *pk {
                schema.id_field = Some(name.clone());
            }
            continue;
        }
        // KV and timeseries list a non-key `id` the server derives.
        if synthetic_id && name == "id" && !pk {
            continue;
        }
        if *pk {
            schema.id_field = Some(name.clone());
        }
        schema.fields.push(SchemaField {
            name: name.clone(),
            ty: parse_type(raw),
            required: !nullable,
            reference: None,
            time_key: raw.split_whitespace().any(|w| w == "TIME_KEY"),
        });
    }
    schema.typed_headers = declared;
    schema.columns = if declared {
        schema
            .fields
            .iter()
            .map(|f| ColumnSpec {
                header: f.name.clone(),
                field: f.name.clone(),
                format: cell_format(f.ty, schema.id_field.as_deref() == Some(&f.name)),
            })
            .collect()
    } else {
        schema
            .id_field
            .iter()
            .map(|id| ColumnSpec {
                header: id.clone(),
                field: id.clone(),
                format: CellFormat::Id,
            })
            .collect()
    };
    schema.filter_fields = schema
        .fields
        .iter()
        .filter(|f| !matches!(f.ty, FieldType::Vector { .. } | FieldType::Geometry { .. }))
        .map(|f| f.name.clone())
        .collect();
    if let Some(id) = &schema.id_field
        && declared
    {
        schema.indexes.push(IndexDef {
            name: "primary".to_string(),
            kind: IndexKind::Primary,
            fields: vec![id.clone()],
            ready: true,
        });
    }
    let mine: Vec<&IndexRow> = indexes
        .iter()
        .filter(|i| i.collection == collection)
        .collect();
    // The server lists an index once it exists and reports no build state:
    // every listed index reads as ready.
    schema.indexes.extend(mine.iter().map(|i| IndexDef {
        name: i.name.clone(),
        kind: index_kind(&i.kind),
        fields: i.fields.clone(),
        ready: true,
    }));
    schema.text_index = mine
        .iter()
        .find(|i| i.kind == "fulltext")
        .map(|i| TextIndex {
            name: i.name.clone(),
            fields: i
                .fields
                .iter()
                .map(|f| (f.clone(), String::new()))
                .collect(),
            ready: true,
            coverage_percent: None,
        });
    schema.vector = mine.iter().find(|i| i.kind == "vector").and_then(|i| {
        let field = i.fields.first()?;
        match schema.field(field)?.ty {
            FieldType::Vector { dimensions } => Some(VectorField {
                field: field.clone(),
                dimensions,
                model: String::new(),
                model_revision: 0,
                normalized: false,
                index: i.name.clone(),
            }),
            _ => None,
        }
    });
    Ok(schema)
}

#[async_trait(?Send)]
impl ExplorerData for NodeDbConnectionService {
    async fn catalog(&self) -> Result<Vec<Collection>, StudioError> {
        let listed = self.table("SHOW COLLECTIONS").await?;
        let names = decode_rows(&listed, &["name", "partition_strategy"], |r| {
            Ok((
                r.field("name")?.to_string(),
                r.field("partition_strategy")? == ARRAY_STRATEGY,
            ))
        })?;
        let indexes = decode_indexes(&self.table("SHOW INDEXES").await?)?;
        let mut out = Vec::with_capacity(names.len());
        // ponytail: one DESCRIBE per collection, because only DESCRIBE names the
        // storage engine. Add a batched source if the server grows one.
        for (name, is_array) in names {
            // An array has no DESCRIBE (42P01) and no index.
            let profile = if is_array {
                Profile::Array
            } else {
                profile_of_describe(
                    &name,
                    &self.table(&format!("DESCRIBE {}", ident(&name)?)).await?,
                )?
            };
            out.push(Collection {
                attached: attached_of(&indexes, &name),
                name,
                profile,
                description: String::new(),
                // SHOW STORAGE FOR reports row_count only after ANALYZE and
                // answers 0 before: not a count, so none.
                rows: None,
            });
        }
        Ok(out)
    }
    async fn collection_schema(&self, collection: &str) -> Result<CollectionSchema, StudioError> {
        // An unknown collection fails here with the server's not-found error.
        // `SHOW INDEXES ON` would answer an empty list instead.
        let collection = ident(collection)?;
        let describe = self.table(&format!("DESCRIBE {collection}")).await?;
        let profile = profile_of_describe(collection, &describe)?;
        let indexes = decode_indexes(&self.table(&format!("SHOW INDEXES ON {collection}")).await?)?;
        schema_of(collection, profile, &describe, &indexes)
    }
    async fn collection_access(&self, _collection: &str) -> Result<CollectionAccess, StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn records(
        &self,
        _collection: &str,
        _query: &RecordQuery,
    ) -> Result<RecordPage, StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn record(&self, _collection: &str, _id: &str) -> Result<Record, StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn insert_record(
        &self,
        _collection: &str,
        _id: Option<&str>,
        _value: Value,
    ) -> Result<Record, StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn update_record(
        &self,
        _collection: &str,
        _id: &str,
        _expected_revision: u64,
        _value: Value,
    ) -> Result<Record, StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn delete_record(
        &self,
        _collection: &str,
        _id: &str,
        _expected_revision: u64,
    ) -> Result<(), StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn edges(&self, _graph: &str, _query: &EdgeQuery) -> Result<EdgePage, StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn delete_edge(&self, _graph: &str, _id: &str) -> Result<(), StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn array_schema(&self, _array: &str) -> Result<ArraySchema, StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn array_slice(
        &self,
        _array: &str,
        _request: &SliceRequest,
    ) -> Result<SliceResult, StudioError> {
        Err(StudioError::NotConnected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(cols: &[&str], rows: &[&[&str]]) -> Table {
        Table {
            columns: cols.iter().map(|s| s.to_string()).collect(),
            rows: rows
                .iter()
                .map(|r| r.iter().map(|s| s.to_string()).collect())
                .collect(),
        }
    }

    const DESCRIBE: &[&str] = &["field", "type", "nullable", "primary_key"];
    const INDEXES: &[&str] = &["index_name", "type", "collection", "fields", "owner"];

    fn idx(name: &str, kind: &str, coll: &str, fields: &str) -> IndexRow {
        IndexRow {
            name: name.into(),
            kind: kind.into(),
            collection: coll.into(),
            fields: vec![fields.into()],
        }
    }

    #[test]
    fn types_parse_with_parameters_and_modifiers() {
        assert_eq!(parse_type("TEXT NOT NULL"), FieldType::String { max: None });
        assert_eq!(
            parse_type("VARCHAR(12)"),
            FieldType::String { max: Some(12) }
        );
        assert_eq!(parse_type("BIGINT"), FieldType::Int64);
        assert_eq!(
            parse_type("DECIMAL(10,2)"),
            FieldType::Decimal {
                precision: 10,
                scale: 2
            }
        );
        assert_eq!(parse_type("VECTOR(3)"), FieldType::Vector { dimensions: 3 });
        assert_eq!(parse_type("TIMESTAMP TIME_KEY"), FieldType::Timestamp);
        assert_eq!(
            parse_type("GEOMETRY SPATIAL_INDEX"),
            FieldType::Geometry { srid: 4326 }
        );
        assert_eq!(parse_type("SOMETHING_NEW"), FieldType::Json);
    }

    #[test]
    fn storage_names_the_profile() {
        let d = table(DESCRIBE, &[&["__storage", "kv", "false", "false"]]);
        assert_eq!(profile_of_describe("c", &d).ok(), Some(Profile::KeyValue));
        let none = table(DESCRIBE, &[&["id", "TEXT", "false", "true"]]);
        assert_eq!(
            profile_of_describe("c", &none).ok(),
            Some(Profile::Document)
        );
        let vector = table(DESCRIBE, &[&["__storage", "vector", "false", "false"]]);
        assert!(profile_of_describe("c", &vector).is_err());
    }

    #[test]
    fn setting_fallback_is_an_error() {
        let t = table(&["setting"], &[&[""]]);
        assert!(matches!(
            profile_of_describe("c", &t),
            Err(StudioError::UnexpectedColumns { .. })
        ));
        assert!(matches!(
            decode_indexes(&t),
            Err(StudioError::UnexpectedColumns { .. })
        ));
    }

    #[test]
    fn strict_schema_carries_fields_and_indexes() {
        let d = table(
            DESCRIBE,
            &[
                &["id", "TEXT PRIMARY KEY", "false", "true"],
                &["body", "TEXT", "true", "false"],
                &["emb", "VECTOR(3)", "true", "false"],
                &["__storage", "document_strict", "false", "false"],
            ],
        );
        let ix = [
            idx("v", "vector", "c", "emb"),
            idx("f", "fulltext", "c", "body"),
            idx("other", "btree", "elsewhere", "x"),
        ];
        let s = schema_of("c", Profile::DocumentStrict, &d, &ix).expect("schema");
        assert_eq!(s.id_field.as_deref(), Some("id"));
        assert_eq!(s.fields.len(), 3);
        assert!(s.fields[0].required && !s.fields[1].required);
        assert_eq!(s.columns[0].format, CellFormat::Id);
        assert_eq!(s.filter_fields, ["id", "body"]);
        assert_eq!(s.indexes.len(), 3, "primary plus the two of `c`");
        assert_eq!(s.vector.as_ref().map(|v| v.dimensions), Some(3));
        assert_eq!(s.text_index.as_ref().map(|t| t.name.as_str()), Some("f"));
    }

    #[test]
    fn schemaless_declares_no_fields_and_ts_marks_its_time_key() {
        let d = table(
            DESCRIBE,
            &[
                &["id", "TEXT", "false", "true"],
                &["document", "JSON", "true", "false"],
                &["__storage", "document_schemaless", "false", "false"],
            ],
        );
        let s = schema_of("c", Profile::Document, &d, &[]).expect("schema");
        assert!(s.fields.is_empty() && !s.typed_headers);
        assert_eq!(s.columns.len(), 1);

        let t = table(
            DESCRIBE,
            &[
                &["id", "TEXT", "false", "false"],
                &["ts", "TIMESTAMP TIME_KEY", "true", "false"],
                &["cpu", "FLOAT", "true", "false"],
                &["__storage", "timeseries", "false", "false"],
            ],
        );
        let s = schema_of("c", Profile::Timeseries, &t, &[]).expect("schema");
        assert_eq!(s.time_key().map(|f| f.name.as_str()), Some("ts"));
        assert!(s.field("id").is_none(), "the derived id is not a field");
    }

    #[test]
    fn attached_reads_index_types_per_collection() {
        let ix = [idx("v", "vector", "a", "e"), idx("f", "fulltext", "b", "t")];
        assert_eq!(attached_of(&ix, "a"), [Attached::Vector]);
        assert_eq!(attached_of(&ix, "b"), [Attached::FullText]);
        assert!(attached_of(&ix, "c").is_empty());
    }

    #[test]
    fn index_fields_drop_the_document_path_prefix() {
        let t = table(INDEXES, &[&["i", "btree", "c", "$.name", "o"]]);
        assert_eq!(decode_indexes(&t).expect("rows")[0].fields, ["name"]);
    }

    #[test]
    fn names_outside_the_identifier_set_are_not_found() {
        assert!(ident("orders_2").is_ok());
        assert!(matches!(ident("a; DROP"), Err(StudioError::NotFound(_))));
        assert!(matches!(ident(""), Err(StudioError::NotFound(_))));
    }

    /// Idempotent: a collection that exists already makes the DDL error, which is fine.
    async fn seed(svc: &NodeDbConnectionService) {
        let c = svc.client(None).expect("client");
        for sql in [
            "CREATE COLLECTION studio_probe_doc",
            "CREATE COLLECTION studio_probe_strict (id TEXT PRIMARY KEY, name TEXT NOT NULL, qty BIGINT) WITH (engine='document_strict')",
            "CREATE COLLECTION studio_probe_kv (key TEXT PRIMARY KEY, value TEXT) WITH (engine='kv')",
            "CREATE COLLECTION studio_probe_col (id BIGINT, name TEXT) WITH (engine='columnar')",
            "CREATE COLLECTION studio_probe_ts (ts TIMESTAMP TIME_KEY, cpu FLOAT) WITH (engine='timeseries')",
            "CREATE COLLECTION studio_probe_sp (id BIGINT, geom GEOMETRY SPATIAL_INDEX) WITH (engine='spatial')",
            "CREATE COLLECTION studio_probe_vec (id TEXT PRIMARY KEY, body TEXT, emb VECTOR(3)) WITH (engine='document_strict')",
            "CREATE VECTOR INDEX studio_probe_vidx ON studio_probe_vec (emb) METRIC cosine DIM 3",
            "CREATE SEARCH INDEX studio_probe_fidx ON studio_probe_vec FIELDS body",
            "CREATE ARRAY studio_probe_arr DIMS (x INT64 [0..100]) ATTRS (a FLOAT64) TILE_EXTENTS (10)",
        ] {
            let _ = c.query(sql).await;
        }
    }

    #[tokio::test]
    #[ignore = "needs a live server: NODEDB_PROBE_ADDR"]
    async fn live_explorer_catalog_has_each_profile() {
        let svc = crate::services::nodedb::service::live().await;
        seed(&svc).await;
        let all = svc.catalog().await.expect("catalog reads");
        let of = |n: &str| {
            all.iter()
                .find(|c| c.name == n)
                .unwrap_or_else(|| panic!("{n} listed"))
        };
        for (name, profile) in [
            ("studio_probe_doc", Profile::Document),
            ("studio_probe_strict", Profile::DocumentStrict),
            ("studio_probe_kv", Profile::KeyValue),
            ("studio_probe_col", Profile::Columnar),
            ("studio_probe_ts", Profile::Timeseries),
            ("studio_probe_sp", Profile::Spatial),
            ("studio_probe_arr", Profile::Array),
        ] {
            assert_eq!(of(name).profile, profile, "{name}");
            assert_eq!(of(name).rows, None, "{name} reports no cheap count");
        }
        assert_eq!(
            of("studio_probe_vec").attached,
            [Attached::FullText, Attached::Vector]
        );
        assert!(of("studio_probe_doc").attached.is_empty());
    }

    #[tokio::test]
    #[ignore = "needs a live server: NODEDB_PROBE_ADDR"]
    async fn live_explorer_schema_maps_describe_and_indexes() {
        let svc = crate::services::nodedb::service::live().await;
        seed(&svc).await;
        let s = svc
            .collection_schema("studio_probe_vec")
            .await
            .expect("schema");
        assert_eq!(s.id_field.as_deref(), Some("id"));
        assert_eq!(
            s.field("emb").map(|f| f.ty),
            Some(FieldType::Vector { dimensions: 3 })
        );
        assert_eq!(
            s.vector.as_ref().map(|v| v.index.as_str()),
            Some("studio_probe_vidx")
        );
        assert_eq!(
            s.text_index.as_ref().map(|t| t.name.as_str()),
            Some("studio_probe_fidx")
        );

        let strict = svc
            .collection_schema("studio_probe_strict")
            .await
            .expect("schema");
        assert!(strict.field("name").is_some_and(|f| f.required));
        assert!(
            strict
                .field("qty")
                .is_some_and(|f| !f.required && f.ty == FieldType::Int64)
        );

        let ts = svc
            .collection_schema("studio_probe_ts")
            .await
            .expect("schema");
        assert_eq!(ts.time_key().map(|f| f.name.as_str()), Some("ts"));
    }

    #[tokio::test]
    #[ignore = "needs a live server: NODEDB_PROBE_ADDR"]
    async fn live_explorer_unknown_collection_is_not_found() {
        let svc = crate::services::nodedb::service::live().await;
        assert!(matches!(
            svc.collection_schema("studio_probe_no_such").await,
            Err(StudioError::NotFound(_))
        ));
    }
}
