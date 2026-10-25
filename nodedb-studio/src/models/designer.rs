//! What the Designer draws: collections with their fields and indexes, and
//! the logical references between them.
//!
//! A collection is the Explorer catalog's `Collection` (one storage profile,
//! attached capabilities) plus its design: fields, indexes, and the settings
//! its profile carries. A reference is a design annotation. NodeDB does not
//! enforce a foreign key, so a reference never becomes constraint DDL.
//!
//! Where a change has been staged but not applied, `DesignSchema::with_pending`
//! lays it over the applied catalog as a draft.

use serde::{Deserialize, Serialize};

use crate::models::collection::{Collection, Profile};
use crate::models::designer_change::{ChangeKind, PendingChange};
use crate::models::schema::{FieldType, IndexKind, SchemaField};

/// Whether an object is in the database or only staged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectState {
    Applied,
    Draft,
}

impl ObjectState {
    pub fn is_draft(self) -> bool {
        self == ObjectState::Draft
    }
}

/// The section title above a document's fields. A strict or columnar
/// collection has typed columns and no title.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldsHeading {
    /// Fields a typeguard enforces.
    Declared,
    /// Fields seen in stored documents. Never read as constraints.
    Observed,
}

impl FieldsHeading {
    pub fn label(self) -> &'static str {
        match self {
            FieldsHeading::Declared => "Declared fields",
            FieldsHeading::Observed => "Observed fields",
        }
    }
}

/// The small marker after a field's type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldFlag {
    Primary,
    /// A document field the typeguard requires.
    Required,
    /// A typed column that rejects null.
    NotNull,
    TimeKey,
}

impl FieldFlag {
    pub fn label(self) -> &'static str {
        match self {
            FieldFlag::Primary => "PK",
            FieldFlag::Required => "required",
            FieldFlag::NotNull => "not null",
            FieldFlag::TimeKey => "time key",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignField {
    pub schema: SchemaField,
    pub primary: bool,
}

impl DesignField {
    pub fn new(name: &str, ty: FieldType) -> Self {
        Self {
            schema: SchemaField {
                name: name.to_string(),
                ty,
                required: false,
                reference: None,
                time_key: false,
            },
            primary: false,
        }
    }

    pub fn required(mut self) -> Self {
        self.schema.required = true;
        self
    }

    pub fn primary(mut self) -> Self {
        self.primary = true;
        self
    }

    pub fn time_key(mut self) -> Self {
        self.schema.time_key = true;
        self
    }

    pub fn name(&self) -> &str {
        &self.schema.name
    }

    pub fn ty(&self) -> FieldType {
        self.schema.ty
    }

    /// The markers a row shows. A primary key implies not null, so it
    /// shows `PK` alone.
    pub fn flags(&self, profile: Profile) -> Vec<FieldFlag> {
        let mut out = Vec::new();
        if self.primary {
            out.push(FieldFlag::Primary);
        } else if self.schema.required && !self.schema.time_key {
            out.push(if profile.enforces_schema() {
                FieldFlag::NotNull
            } else {
                FieldFlag::Required
            });
        }
        if self.schema.time_key {
            out.push(FieldFlag::TimeKey);
        }
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VectorMetric {
    Cosine,
    L2,
    InnerProduct,
}

impl VectorMetric {
    pub const ALL: [VectorMetric; 3] = [
        VectorMetric::Cosine,
        VectorMetric::L2,
        VectorMetric::InnerProduct,
    ];

    /// The word `CREATE VECTOR INDEX ... METRIC` takes.
    pub fn ddl(self) -> &'static str {
        match self {
            VectorMetric::Cosine => "cosine",
            VectorMetric::L2 => "l2",
            VectorMetric::InnerProduct => "ip",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorParams {
    pub dimensions: u32,
    pub metric: VectorMetric,
}

/// The index kinds a form offers. Spatial and sparse indexes derive their
/// own names on the server, so the Designer does not create them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexChoice {
    Secondary,
    Unique,
    FullText,
    Vector,
}

impl IndexChoice {
    pub const ALL: [IndexChoice; 4] = [
        IndexChoice::Secondary,
        IndexChoice::Unique,
        IndexChoice::FullText,
        IndexChoice::Vector,
    ];

    pub fn label(self) -> &'static str {
        match self {
            IndexChoice::Secondary => "Secondary",
            IndexChoice::Unique => "Unique",
            IndexChoice::FullText => "Full-text",
            IndexChoice::Vector => "Vector",
        }
    }

    pub fn kind(self) -> IndexKind {
        match self {
            IndexChoice::Secondary | IndexChoice::Unique => IndexKind::Secondary,
            IndexChoice::FullText => IndexKind::FullText,
            IndexChoice::Vector => IndexKind::Vector,
        }
    }

    pub fn unique(self) -> bool {
        self == IndexChoice::Unique
    }

    /// Whether a field of type `ty` can carry this index.
    pub fn accepts(self, ty: FieldType) -> bool {
        match self {
            IndexChoice::Secondary | IndexChoice::Unique => !matches!(
                ty,
                FieldType::Object
                    | FieldType::Array
                    | FieldType::Json
                    | FieldType::Vector { .. }
                    | FieldType::Geometry { .. }
                    | FieldType::Bytes
            ),
            IndexChoice::FullText => matches!(ty, FieldType::String { .. }),
            IndexChoice::Vector => matches!(ty, FieldType::Vector { .. }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignIndex {
    pub name: String,
    pub collection: String,
    pub kind: IndexKind,
    pub unique: bool,
    pub fields: Vec<String>,
    /// Set for a vector index.
    pub vector: Option<VectorParams>,
    pub state: ObjectState,
}

impl DesignIndex {
    pub fn choice(&self) -> Option<IndexChoice> {
        match (self.kind, self.unique) {
            (IndexKind::Secondary, false) => Some(IndexChoice::Secondary),
            (IndexKind::Secondary, true) => Some(IndexChoice::Unique),
            (IndexKind::FullText, _) => Some(IndexChoice::FullText),
            (IndexKind::Vector, _) => Some(IndexChoice::Vector),
            (IndexKind::Primary | IndexKind::Spatial, _) => None,
        }
    }

    /// The small text beside the name on a canvas row.
    pub fn summary(&self) -> String {
        let field = self.fields.join(", ");
        match (self.kind, self.unique, self.vector) {
            (IndexKind::Vector, _, Some(v)) => {
                format!("Vector · {}d · {}", v.dimensions, v.metric.ddl())
            }
            (IndexKind::Vector, _, None) => "Vector".to_string(),
            (IndexKind::FullText, _, _) => format!("Full-text · {field}"),
            (IndexKind::Secondary, true, _) => "unique".to_string(),
            (IndexKind::Secondary, false, _) => format!("Secondary · {field}"),
            (IndexKind::Primary, _, _) => "Primary".to_string(),
            (IndexKind::Spatial, _, _) => format!("Spatial · {field}"),
        }
    }
}

/// A design annotation: `from` holds the id of a record in `to`. The server
/// does not check it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogicalReference {
    pub id: String,
    pub from_collection: String,
    pub from_field: String,
    pub to_collection: String,
    pub to_field: String,
    pub state: ObjectState,
}

impl LogicalReference {
    /// `users.id → orders.customer_id`, in the direction the arrow reads.
    pub fn label(&self) -> String {
        format!(
            "{}.{} → {}.{}",
            self.from_collection, self.from_field, self.to_collection, self.to_field
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignCollection {
    pub collection: Collection,
    pub heading: Option<FieldsHeading>,
    pub fields: Vec<DesignField>,
    pub indexes: Vec<DesignIndex>,
    /// The settings line of a profile that has them: `Partition 1h · Retention 90d`.
    pub settings: Option<String>,
    pub state: ObjectState,
}

impl DesignCollection {
    pub fn name(&self) -> &str {
        &self.collection.name
    }

    /// This collection as the database holds it: itself and its indexes no
    /// longer drafts.
    pub fn applied(mut self) -> Self {
        self.state = ObjectState::Applied;
        for i in &mut self.indexes {
            i.state = ObjectState::Applied;
        }
        self
    }

    pub fn profile(&self) -> Profile {
        self.collection.profile
    }

    pub fn field(&self, name: &str) -> Option<&DesignField> {
        self.fields.iter().find(|f| f.name() == name)
    }

    pub fn index(&self, name: &str) -> Option<&DesignIndex> {
        self.indexes.iter().find(|i| i.name == name)
    }

    /// The fields an index of `choice` can sit on.
    pub fn fields_for(&self, choice: IndexChoice) -> Vec<&DesignField> {
        self.fields
            .iter()
            .filter(|f| choice.accepts(f.ty()))
            .collect()
    }
}

/// The catalog as the Designer draws it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DesignSchema {
    pub database: String,
    /// The collections drawn on the canvas.
    pub collections: Vec<DesignCollection>,
    /// Collections of the database the canvas does not draw. Their names
    /// are taken all the same.
    pub off_canvas: Vec<Collection>,
    pub references: Vec<LogicalReference>,
}

impl DesignSchema {
    /// Whether `name` belongs to a collection of the database, drawn or not.
    pub fn name_taken(&self, name: &str) -> bool {
        self.collection(name).is_some() || self.off_canvas.iter().any(|c| c.name == name)
    }

    pub fn collection(&self, name: &str) -> Option<&DesignCollection> {
        self.collections.iter().find(|c| c.name() == name)
    }

    /// The collection that owns index `name`. Index names are unique
    /// across the database.
    pub fn index(&self, name: &str) -> Option<&DesignIndex> {
        self.collections.iter().find_map(|c| c.index(name))
    }

    pub fn reference(&self, id: &str) -> Option<&LogicalReference> {
        self.references.iter().find(|r| r.id == id)
    }

    /// This schema with the staged changes laid over it as drafts, in
    /// staging order.
    pub fn with_pending(&self, pending: &[PendingChange]) -> DesignSchema {
        let mut out = self.clone();
        for change in pending {
            match &change.kind {
                ChangeKind::CreateCollection(new) => {
                    // A name already taken keeps the first collection.
                    if out.collection(&new.name).is_none() {
                        out.collections.push(new.to_design());
                    }
                }
                ChangeKind::CreateIndex(index) => {
                    if let Some(owner) = out
                        .collections
                        .iter_mut()
                        .find(|c| c.name() == index.collection)
                    {
                        owner.indexes.push(DesignIndex {
                            state: ObjectState::Draft,
                            ..index.clone()
                        });
                    }
                }
                ChangeKind::AddReference(r) => out.references.push(LogicalReference {
                    id: change.id.clone(),
                    state: ObjectState::Draft,
                    ..r.clone()
                }),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_primary_key_shows_pk_and_nothing_else() {
        let f = DesignField::new("id", FieldType::Uuid).required().primary();
        assert_eq!(f.flags(Profile::DocumentStrict), [FieldFlag::Primary]);
    }

    #[test]
    fn required_reads_by_profile() {
        let f = DesignField::new("email", FieldType::String { max: None }).required();
        assert_eq!(f.flags(Profile::Document), [FieldFlag::Required]);
        assert_eq!(f.flags(Profile::DocumentStrict), [FieldFlag::NotNull]);
    }

    #[test]
    fn a_text_index_accepts_text_only() {
        assert!(IndexChoice::FullText.accepts(FieldType::String { max: None }));
        assert!(!IndexChoice::FullText.accepts(FieldType::Int64));
        assert!(IndexChoice::Vector.accepts(FieldType::Vector { dimensions: 4 }));
        assert!(!IndexChoice::Secondary.accepts(FieldType::Json));
    }

    #[test]
    fn index_summaries_match_the_canvas() {
        let vector = DesignIndex {
            name: "v".into(),
            collection: "a".into(),
            kind: IndexKind::Vector,
            unique: false,
            fields: vec!["embedding".into()],
            vector: Some(VectorParams {
                dimensions: 768,
                metric: VectorMetric::Cosine,
            }),
            state: ObjectState::Applied,
        };
        assert_eq!(vector.summary(), "Vector · 768d · cosine");
        let text = DesignIndex {
            kind: IndexKind::FullText,
            vector: None,
            fields: vec!["body".into()],
            ..vector
        };
        assert_eq!(text.summary(), "Full-text · body");
    }
}
