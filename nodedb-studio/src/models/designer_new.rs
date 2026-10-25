//! The Create collection form: what the user types, how it maps to a storage
//! profile, and what it must satisfy before it can be staged.

use serde::{Deserialize, Serialize};

use crate::models::collection::{Attached, Collection, Profile};
use crate::models::designer::{
    DesignCollection, DesignField, DesignIndex, FieldsHeading, LogicalReference, ObjectState,
    VectorMetric, VectorParams,
};
use crate::models::designer_change::ChangeKind;
use crate::models::schema::{FieldType, IndexKind};

/// The base storage type. `Collection` profiles refine it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Storage {
    Document,
    Columnar,
    KeyValue,
}

impl Storage {
    pub const ALL: [Storage; 3] = [Storage::Document, Storage::Columnar, Storage::KeyValue];

    pub fn label(self) -> &'static str {
        match self {
            Storage::Document => "Document",
            Storage::Columnar => "Columnar",
            Storage::KeyValue => "KV",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchemaMode {
    Flexible,
    Strict,
}

impl SchemaMode {
    pub const ALL: [SchemaMode; 2] = [SchemaMode::Flexible, SchemaMode::Strict];

    pub fn label(self) -> &'static str {
        match self {
            SchemaMode::Flexible => "Flexible",
            SchemaMode::Strict => "Strict",
        }
    }
}

/// The types the field editor offers.
pub const FIELD_TYPES: [FieldType; 11] = [
    FieldType::Uuid,
    FieldType::String { max: None },
    FieldType::Int64,
    FieldType::Float64,
    FieldType::Decimal {
        precision: 10,
        scale: 2,
    },
    FieldType::Bool,
    FieldType::Timestamp,
    FieldType::Json,
    FieldType::Vector { dimensions: 384 },
    FieldType::Vector { dimensions: 768 },
    FieldType::Vector { dimensions: 1536 },
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewField {
    /// Stable key for the editor row. Not part of the DDL.
    pub id: u32,
    pub name: String,
    pub ty: FieldType,
    pub required: bool,
    pub primary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintKind {
    Unique,
    ForeignKey,
    Check,
}

impl ConstraintKind {
    pub const ALL: [ConstraintKind; 3] = [
        ConstraintKind::Unique,
        ConstraintKind::ForeignKey,
        ConstraintKind::Check,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ConstraintKind::Unique => "Unique",
            ConstraintKind::ForeignKey => "Foreign key",
            ConstraintKind::Check => "Check",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Constraint {
    /// A unique index over `fields`.
    Unique { id: u32, fields: Vec<String> },
    /// A logical reference. NodeDB does not enforce it, so it stages as
    /// workspace metadata and never as DDL.
    ForeignKey {
        id: u32,
        field: String,
        to_collection: String,
        to_field: String,
    },
    /// A `CHECK` expression over the record.
    Check { id: u32, expression: String },
}

impl Constraint {
    pub fn id(&self) -> u32 {
        match self {
            Constraint::Unique { id, .. }
            | Constraint::ForeignKey { id, .. }
            | Constraint::Check { id, .. } => *id,
        }
    }

    pub fn kind(&self) -> ConstraintKind {
        match self {
            Constraint::Unique { .. } => ConstraintKind::Unique,
            Constraint::ForeignKey { .. } => ConstraintKind::ForeignKey,
            Constraint::Check { .. } => ConstraintKind::Check,
        }
    }
}

/// The Create collection form's value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewCollection {
    pub name: String,
    pub database: String,
    pub storage: Storage,
    pub schema: SchemaMode,
    pub fields: Vec<NewField>,
    pub constraints: Vec<Constraint>,
    pub attach: Vec<Attached>,
    /// The text field the Full-text capability indexes.
    pub text_field: String,
    /// The vector field the Vector capability indexes.
    pub vector_field: String,
    next_id: u32,
}

impl NewCollection {
    /// An empty form with an `id` primary key, for `database`.
    pub fn blank(database: &str) -> Self {
        let mut form = Self {
            name: String::new(),
            database: database.to_string(),
            storage: Storage::Document,
            schema: SchemaMode::Flexible,
            fields: Vec::new(),
            constraints: Vec::new(),
            attach: Vec::new(),
            text_field: String::new(),
            vector_field: String::new(),
            next_id: 1,
        };
        form.add_field("id", FieldType::Uuid, true, true);
        form
    }

    fn take_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn add_field(&mut self, name: &str, ty: FieldType, required: bool, primary: bool) {
        let id = self.take_id();
        self.fields.push(NewField {
            id,
            name: name.to_string(),
            ty,
            required,
            primary,
        });
    }

    pub fn add_constraint(&mut self, kind: ConstraintKind) {
        let id = self.take_id();
        let first = self.fields.first().map(|f| f.name.clone());
        self.constraints.push(match kind {
            ConstraintKind::Unique => Constraint::Unique {
                id,
                fields: first.into_iter().collect(),
            },
            ConstraintKind::ForeignKey => Constraint::ForeignKey {
                id,
                field: first.unwrap_or_default(),
                to_collection: String::new(),
                to_field: String::new(),
            },
            ConstraintKind::Check => Constraint::Check {
                id,
                expression: String::new(),
            },
        });
    }

    /// The storage profile this form creates.
    pub fn profile(&self) -> Profile {
        match (self.storage, self.schema) {
            (Storage::Document, SchemaMode::Flexible) => Profile::Document,
            (Storage::Document, SchemaMode::Strict) => Profile::DocumentStrict,
            (Storage::Columnar, _) => Profile::Columnar,
            (Storage::KeyValue, _) => Profile::KeyValue,
        }
    }

    /// Columnar and key-value storage have one schema mode. Call after the
    /// storage changes so the pair stays one the server has.
    pub fn settle_schema(&mut self) {
        self.schema = match self.storage {
            Storage::Document => self.schema,
            Storage::Columnar | Storage::KeyValue => SchemaMode::Strict,
        };
        // Capabilities attach to documents only.
        if self.storage != Storage::Document {
            self.attach.clear();
        }
    }

    /// Whether the user can pick the schema mode.
    pub fn schema_choice(&self) -> bool {
        self.storage == Storage::Document
    }

    /// Turn a capability on or off. On, it picks the first field it can
    /// index.
    pub fn set_attached(&mut self, capability: Attached, on: bool) {
        self.attach.retain(|a| *a != capability);
        if on {
            self.attach.push(capability);
        }
        self.settle_capability_fields();
    }

    /// Point each capability at a field that still exists and fits.
    pub fn settle_capability_fields(&mut self) {
        let text: Vec<String> = self.names_of(|t| matches!(t, FieldType::String { .. }));
        let vector: Vec<String> = self.names_of(|t| matches!(t, FieldType::Vector { .. }));
        if !text.contains(&self.text_field) {
            self.text_field = text.first().cloned().unwrap_or_default();
        }
        if !vector.contains(&self.vector_field) {
            self.vector_field = vector.first().cloned().unwrap_or_default();
        }
    }

    pub fn names_of(&self, fits: impl Fn(FieldType) -> bool) -> Vec<String> {
        self.fields
            .iter()
            .filter(|f| !f.name.is_empty() && fits(f.ty))
            .map(|f| f.name.clone())
            .collect()
    }

    pub fn attached(&self, capability: Attached) -> bool {
        self.attach.contains(&capability)
    }

    /// The changes staging this form makes: the collection, then one
    /// reference per foreign key. The collection keeps its unique and check
    /// constraints.
    pub fn into_changes(self) -> Vec<ChangeKind> {
        let name = self.name.clone();
        let refs: Vec<ChangeKind> = self
            .constraints
            .iter()
            .filter_map(|c| match c {
                Constraint::ForeignKey {
                    field,
                    to_collection,
                    to_field,
                    ..
                } => Some(ChangeKind::AddReference(LogicalReference {
                    id: String::new(),
                    from_collection: name.clone(),
                    from_field: field.clone(),
                    to_collection: to_collection.clone(),
                    to_field: to_field.clone(),
                    state: ObjectState::Draft,
                })),
                Constraint::Unique { .. } | Constraint::Check { .. } => None,
            })
            .collect();
        let mut own = self;
        own.constraints
            .retain(|c| c.kind() != ConstraintKind::ForeignKey);
        let mut out = vec![ChangeKind::CreateCollection(own)];
        out.extend(refs);
        out
    }

    /// The index name a unique constraint or capability creates.
    pub fn index_name(&self, fields: &[String]) -> String {
        format!("idx_{}_{}", self.name, fields.join("_"))
    }

    /// The collection as a draft on the canvas.
    pub fn to_design(&self) -> DesignCollection {
        let profile = self.profile();
        let fields: Vec<DesignField> = self
            .fields
            .iter()
            .map(|f| {
                let mut d = DesignField::new(&f.name, f.ty);
                d.schema.required = f.required || f.primary;
                d.primary = f.primary;
                d
            })
            .collect();
        let mut indexes: Vec<DesignIndex> = Vec::new();
        let mut push = |name: String, kind: IndexKind, unique: bool, field: &str| {
            let vector = self
                .fields
                .iter()
                .find(|f| f.name == field)
                .and_then(|f| match f.ty {
                    FieldType::Vector { dimensions } if kind == IndexKind::Vector => {
                        Some(VectorParams {
                            dimensions,
                            metric: VectorMetric::Cosine,
                        })
                    }
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
                    | FieldType::Geometry { .. }
                    | FieldType::Bytes => None,
                });
            indexes.push(DesignIndex {
                name,
                collection: self.name.clone(),
                kind,
                unique,
                fields: vec![field.to_string()],
                vector,
                state: ObjectState::Draft,
            });
        };
        for c in &self.constraints {
            if let Constraint::Unique { fields, .. } = c
                && let Some(first) = fields.first()
            {
                push(self.index_name(fields), IndexKind::Secondary, true, first);
            }
        }
        if self.attached(Attached::FullText) && !self.text_field.is_empty() {
            let name = self.index_name(std::slice::from_ref(&self.text_field));
            push(name, IndexKind::FullText, false, &self.text_field);
        }
        if self.attached(Attached::Vector) && !self.vector_field.is_empty() {
            let name = self.index_name(std::slice::from_ref(&self.vector_field));
            push(name, IndexKind::Vector, false, &self.vector_field);
        }
        DesignCollection {
            collection: Collection {
                name: self.name.clone(),
                profile,
                attached: self.attach.clone(),
                description: String::new(),
                rows: Some(0),
            },
            heading: (profile == Profile::Document).then_some(FieldsHeading::Declared),
            fields,
            indexes,
            settings: None,
            state: ObjectState::Draft,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(name: &str) -> NewCollection {
        NewCollection {
            name: name.into(),
            ..NewCollection::blank("analytics")
        }
    }

    #[test]
    fn storage_picks_the_profile() {
        let mut f = named("a");
        assert_eq!(f.profile(), Profile::Document);
        f.schema = SchemaMode::Strict;
        assert_eq!(f.profile(), Profile::DocumentStrict);
        f.storage = Storage::Columnar;
        f.settle_schema();
        assert_eq!(f.profile(), Profile::Columnar);
        f.storage = Storage::KeyValue;
        assert_eq!(f.profile(), Profile::KeyValue);
        assert!(!f.schema_choice());
    }

    #[test]
    fn staging_splits_references_from_the_collection() {
        let mut f = named("shipments");
        f.add_field("order_id", FieldType::Uuid, true, false);
        f.add_constraint(ConstraintKind::ForeignKey);
        f.add_constraint(ConstraintKind::Unique);
        let changes = f.into_changes();
        assert_eq!(changes.len(), 2);
        match (&changes[0], &changes[1]) {
            (ChangeKind::CreateCollection(c), ChangeKind::AddReference(r)) => {
                assert_eq!(c.constraints.len(), 1, "only the unique constraint stays");
                assert_eq!(r.from_collection, "shipments");
            }
            other => panic!("unexpected changes {other:?}"),
        }
    }

    #[test]
    fn capabilities_pick_a_field_that_fits() {
        let mut f = named("articles");
        f.add_field("body", FieldType::String { max: None }, false, false);
        f.add_field(
            "embedding",
            FieldType::Vector { dimensions: 768 },
            false,
            false,
        );
        f.set_attached(Attached::FullText, true);
        f.set_attached(Attached::Vector, true);
        assert_eq!(
            (f.text_field.as_str(), f.vector_field.as_str()),
            ("body", "embedding")
        );
        let design = f.to_design();
        assert_eq!(design.indexes.len(), 2);
        assert!(design.indexes.iter().all(|i| i.state.is_draft()));
        assert_eq!(design.indexes[1].vector.map(|v| v.dimensions), Some(768));
    }
}
