//! What a collection declares about its records: typed fields, the columns
//! a table shows, indexes, and the capability facts the inspector states.
//!
//! The schema comes from the server's catalog. Studio never infers a type
//! from a value: a column of timestamps is a timestamp column because the
//! catalog says so.

use serde::{Deserialize, Serialize};

/// The declared type of one field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldType {
    /// Text, optionally capped at `max` characters.
    String {
        max: Option<u32>,
    },
    Int64,
    Float64,
    Decimal {
        precision: u8,
        scale: u8,
    },
    Bool,
    Timestamp,
    Datetime,
    Uuid,
    Object,
    Array,
    /// Any JSON value: a key-value entry's value.
    Json,
    /// A dense embedding of `dimensions` floats.
    Vector {
        dimensions: u32,
    },
    Geometry {
        srid: u32,
    },
    Bytes,
}

impl FieldType {
    pub fn label(self) -> String {
        match self {
            FieldType::String { max: None } => "string".to_string(),
            FieldType::String { max: Some(n) } => format!("string({n})"),
            FieldType::Int64 => "int64".to_string(),
            FieldType::Float64 => "float64".to_string(),
            FieldType::Decimal { precision, scale } => format!("decimal({precision},{scale})"),
            FieldType::Bool => "bool".to_string(),
            FieldType::Timestamp => "timestamp".to_string(),
            FieldType::Datetime => "datetime".to_string(),
            FieldType::Uuid => "uuid".to_string(),
            FieldType::Object => "object".to_string(),
            FieldType::Array => "array".to_string(),
            FieldType::Json => "json".to_string(),
            FieldType::Vector { dimensions } => format!("vector({dimensions})"),
            FieldType::Geometry { srid } => format!("geometry(EPSG:{srid})"),
            FieldType::Bytes => "bytes".to_string(),
        }
    }

    /// Whether a column of this type reads right-aligned.
    pub fn numeric(self) -> bool {
        match self {
            FieldType::Int64 | FieldType::Float64 | FieldType::Decimal { .. } => true,
            FieldType::String { .. }
            | FieldType::Bool
            | FieldType::Timestamp
            | FieldType::Datetime
            | FieldType::Uuid
            | FieldType::Object
            | FieldType::Array
            | FieldType::Json
            | FieldType::Vector { .. }
            | FieldType::Geometry { .. }
            | FieldType::Bytes => false,
        }
    }

    /// The SQL type name a canvas row shows: parameters appear only when
    /// they change what the column holds, as for a vector's dimension.
    pub fn keyword(self) -> String {
        match self {
            FieldType::String { .. } => "TEXT".to_string(),
            FieldType::Int64 => "INT".to_string(),
            FieldType::Float64 => "FLOAT".to_string(),
            FieldType::Decimal { .. } => "DECIMAL".to_string(),
            FieldType::Bool => "BOOL".to_string(),
            FieldType::Timestamp | FieldType::Datetime => "TIMESTAMP".to_string(),
            FieldType::Uuid => "UUID".to_string(),
            FieldType::Object => "OBJECT".to_string(),
            FieldType::Array => "ARRAY".to_string(),
            FieldType::Json => "JSON".to_string(),
            FieldType::Vector { dimensions } => format!("VECTOR({dimensions})"),
            FieldType::Geometry { .. } => "GEOMETRY".to_string(),
            FieldType::Bytes => "BYTES".to_string(),
        }
    }

    /// The type as a `CREATE COLLECTION` column declares it.
    pub fn ddl(self) -> String {
        match self {
            FieldType::String { max: Some(n) } => format!("VARCHAR({n})"),
            FieldType::Decimal { precision, scale } => format!("DECIMAL({precision},{scale})"),
            FieldType::String { max: None }
            | FieldType::Int64
            | FieldType::Float64
            | FieldType::Bool
            | FieldType::Timestamp
            | FieldType::Datetime
            | FieldType::Uuid
            | FieldType::Object
            | FieldType::Array
            | FieldType::Json
            | FieldType::Vector { .. }
            | FieldType::Geometry { .. }
            | FieldType::Bytes => self.keyword(),
        }
    }
}

/// One declared field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaField {
    pub name: String,
    pub ty: FieldType,
    pub required: bool,
    /// The collection a logical reference points to. A reference is not an
    /// enforced foreign key.
    pub reference: Option<String>,
    /// The event-time field of a timeseries collection.
    pub time_key: bool,
}

/// How a table cell renders its field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CellFormat {
    Text,
    /// Right-aligned digits, as stored.
    Number,
    Timestamp,
    /// `Point(lon, lat)`.
    Geometry,
    /// The geometry field's SRID as `EPSG:n`.
    Srid,
    /// The first components of an embedding, `[0.021, -0.104, …]`.
    Embedding,
    /// A byte count, `2.1 KB`.
    Bytes,
    /// Seconds to expiry, `14m`, or `-` for none.
    Ttl,
    /// A value's compact JSON.
    Preview,
    /// Text led by a status dot.
    StatusDot,
    /// Text in a colored badge.
    StatusBadge,
    /// A record id that opens the record.
    Id,
}

/// One table column.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnSpec {
    pub header: String,
    /// The record field the cell reads. Empty for a column the server
    /// derives, such as the SRID of a geometry field.
    pub field: String,
    pub format: CellFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndexKind {
    Primary,
    Secondary,
    FullText,
    Vector,
    Spatial,
}

impl IndexKind {
    pub fn label(self) -> &'static str {
        match self {
            IndexKind::Primary => "Primary",
            IndexKind::Secondary => "Secondary",
            IndexKind::FullText => "Full-text",
            IndexKind::Vector => "Vector",
            IndexKind::Spatial => "Spatial",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexDef {
    pub name: String,
    pub kind: IndexKind,
    pub fields: Vec<String>,
    pub ready: bool,
}

/// The full-text index over a collection's text fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextIndex {
    pub name: String,
    /// `(field, analyzer)` per indexed field.
    pub fields: Vec<(String, String)>,
    pub ready: bool,
    /// Share of source records the index covers. `None` when the server
    /// does not report it: Ready alone never proves full coverage.
    pub coverage_percent: Option<u8>,
}

/// The vector attribute of a collection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorField {
    pub field: String,
    pub dimensions: u32,
    pub model: String,
    pub model_revision: u32,
    pub normalized: bool,
    pub index: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionSchema {
    pub collection: String,
    pub fields: Vec<SchemaField>,
    /// The field that holds the record id. `None` when the server assigns
    /// the id and no field carries it.
    pub id_field: Option<String>,
    /// The table's columns, in display order.
    pub columns: Vec<ColumnSpec>,
    pub indexes: Vec<IndexDef>,
    pub default_page_size: usize,
    /// The sort a fresh view applies, as a field name and a direction.
    pub default_sort: Option<(String, bool)>,
    /// Fields the filter editor offers.
    pub filter_fields: Vec<String>,
    /// The graph binding of a collection with the Graph capability.
    pub graph_binding: Option<String>,
    pub text_index: Option<TextIndex>,
    pub vector: Option<VectorField>,
    /// Whether column headers carry the field type.
    pub typed_headers: bool,
}

impl CollectionSchema {
    /// A schema with no fields, columns, or indexes, and a page size of ten.
    pub fn new(collection: &str) -> Self {
        Self {
            collection: collection.to_string(),
            fields: Vec::new(),
            id_field: None,
            columns: Vec::new(),
            indexes: Vec::new(),
            default_page_size: 10,
            default_sort: None,
            filter_fields: Vec::new(),
            graph_binding: None,
            text_index: None,
            vector: None,
            typed_headers: false,
        }
    }

    pub fn field(&self, name: &str) -> Option<&SchemaField> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// The event-time field, for a timeseries collection.
    pub fn time_key(&self) -> Option<&SchemaField> {
        self.fields.iter().find(|f| f.time_key)
    }

    /// The geometry field, for a spatial collection.
    pub fn geometry(&self) -> Option<(&SchemaField, u32)> {
        self.fields.iter().find_map(|f| match f.ty {
            FieldType::Geometry { srid } => Some((f, srid)),
            FieldType::String { .. }
            | FieldType::Int64
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
            | FieldType::Bytes => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_labels_carry_their_parameters() {
        assert_eq!(
            FieldType::Decimal {
                precision: 10,
                scale: 2
            }
            .label(),
            "decimal(10,2)"
        );
        assert_eq!(FieldType::String { max: Some(3) }.label(), "string(3)");
        assert_eq!(
            FieldType::Geometry { srid: 4326 }.label(),
            "geometry(EPSG:4326)"
        );
    }

    #[test]
    fn only_numbers_align_right() {
        assert!(FieldType::Int64.numeric());
        assert!(
            FieldType::Decimal {
                precision: 10,
                scale: 2
            }
            .numeric()
        );
        assert!(!FieldType::Timestamp.numeric());
    }
}
