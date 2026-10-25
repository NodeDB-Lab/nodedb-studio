//! The applied schema the Designer opens on: the four collections of the
//! design, with the collection facts the Explorer catalog holds.

use crate::data::mock::collections;
use crate::models::collection::Collection;
use crate::models::designer::{
    DesignCollection, DesignField, DesignIndex, DesignSchema, FieldsHeading, LogicalReference,
    ObjectState, VectorMetric, VectorParams,
};
use crate::models::schema::{
    CellFormat, CollectionSchema, ColumnSpec, FieldType, IndexDef, IndexKind,
};

/// The database the mock designs.
pub const DATABASE: &str = "analytics";

fn text() -> FieldType {
    FieldType::String { max: None }
}

fn index(name: &str, collection: &str, kind: IndexKind, unique: bool, field: &str) -> DesignIndex {
    DesignIndex {
        name: name.to_string(),
        collection: collection.to_string(),
        kind,
        unique,
        fields: vec![field.to_string()],
        vector: None,
        state: ObjectState::Applied,
    }
}

fn users(collection: Collection) -> DesignCollection {
    DesignCollection {
        collection,
        heading: Some(FieldsHeading::Declared),
        fields: vec![
            DesignField::new("id", text()),
            DesignField::new("email", text()).required(),
            DesignField::new("country", text()),
        ],
        indexes: vec![index(
            "idx_users_email",
            "users",
            IndexKind::Secondary,
            true,
            "email",
        )],
        settings: None,
        state: ObjectState::Applied,
    }
}

fn articles(collection: Collection) -> DesignCollection {
    let mut embedding = index(
        "idx_articles_embedding",
        "articles",
        IndexKind::Vector,
        false,
        "embedding",
    );
    embedding.vector = Some(VectorParams {
        dimensions: 768,
        metric: VectorMetric::Cosine,
    });
    DesignCollection {
        collection,
        heading: Some(FieldsHeading::Observed),
        fields: vec![
            DesignField::new("id", text()),
            DesignField::new("title", text()),
            DesignField::new("body", text()),
            DesignField::new("embedding", FieldType::Vector { dimensions: 768 }),
        ],
        indexes: vec![embedding],
        settings: None,
        state: ObjectState::Applied,
    }
}

fn orders(collection: Collection) -> DesignCollection {
    let decimal = FieldType::Decimal {
        precision: 10,
        scale: 2,
    };
    DesignCollection {
        collection,
        heading: None,
        fields: vec![
            DesignField::new("id", text()).required().primary(),
            DesignField::new("customer_id", text()),
            DesignField::new("total", decimal).required(),
            DesignField::new("created_at", FieldType::Timestamp),
        ],
        indexes: vec![],
        settings: None,
        state: ObjectState::Applied,
    }
}

fn sales(collection: Collection) -> DesignCollection {
    let decimal = FieldType::Decimal {
        precision: 12,
        scale: 2,
    };
    DesignCollection {
        collection,
        heading: None,
        fields: vec![
            DesignField::new("sold_at", FieldType::Timestamp)
                .required()
                .time_key(),
            DesignField::new("store_id", text()),
            DesignField::new("revenue", decimal),
            DesignField::new("currency", text()),
        ],
        indexes: vec![],
        settings: Some("Partition 1h · Retention 90d".to_string()),
        state: ObjectState::Applied,
    }
}

/// The reference the design draws: an orders row names its customer.
pub fn seed_references() -> Vec<LogicalReference> {
    vec![LogicalReference {
        id: "ref-1".to_string(),
        from_collection: "orders".to_string(),
        from_field: "customer_id".to_string(),
        to_collection: "users".to_string(),
        to_field: "id".to_string(),
        state: ObjectState::Applied,
    }]
}

/// The applied schema before any change, in canvas order.
pub fn design_schema() -> DesignSchema {
    type Build = fn(Collection) -> DesignCollection;
    let builders: [(&str, Build); 4] = [
        ("users", users),
        ("articles", articles),
        ("orders", orders),
        ("sales", sales),
    ];
    let catalog = collections();
    DesignSchema {
        database: DATABASE.to_string(),
        collections: builders
            .into_iter()
            .filter_map(|(name, build)| {
                catalog
                    .iter()
                    .find(|c| c.name == name)
                    .map(|c| build(c.clone()))
            })
            .collect(),
        off_canvas: catalog
            .iter()
            .filter(|c| !builders.iter().any(|(name, _)| *name == c.name))
            .cloned()
            .collect(),
        references: seed_references(),
    }
}

/// What the Explorer reads about a collection the Designer drew: its
/// fields as columns, its indexes, and no records.
pub fn explorer_schema(c: &DesignCollection) -> CollectionSchema {
    let format = |ty: FieldType| match ty {
        FieldType::Timestamp | FieldType::Datetime => CellFormat::Timestamp,
        FieldType::Int64 | FieldType::Float64 | FieldType::Decimal { .. } => CellFormat::Number,
        FieldType::Vector { .. } => CellFormat::Embedding,
        FieldType::Object | FieldType::Array | FieldType::Json => CellFormat::Preview,
        FieldType::String { .. }
        | FieldType::Bool
        | FieldType::Uuid
        | FieldType::Geometry { .. }
        | FieldType::Bytes => CellFormat::Text,
    };
    let id_field = c
        .fields
        .iter()
        .find(|f| f.primary)
        .map(|f| f.name().to_string());
    let mut schema = CollectionSchema::new(c.name());
    schema.fields = c.fields.iter().map(|f| f.schema.clone()).collect();
    schema.columns = c
        .fields
        .iter()
        .map(|f| ColumnSpec {
            header: f.name().to_string(),
            field: f.name().to_string(),
            format: if id_field.as_deref() == Some(f.name()) {
                CellFormat::Id
            } else {
                format(f.ty())
            },
        })
        .collect();
    schema.filter_fields = c.fields.iter().map(|f| f.name().to_string()).collect();
    schema.indexes = c
        .indexes
        .iter()
        .map(|i| IndexDef {
            name: i.name.clone(),
            kind: i.kind,
            fields: i.fields.clone(),
            ready: true,
        })
        .collect();
    schema.id_field = id_field;
    schema
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_design_holds_its_four_collections() {
        let names: Vec<String> = design_schema()
            .collections
            .iter()
            .map(|c| c.name().to_string())
            .collect();
        assert_eq!(names, ["users", "articles", "orders", "sales"]);
    }

    #[test]
    fn collections_match_the_explorer_catalog() {
        let all = collections();
        for c in design_schema().collections {
            let listed = all.iter().find(|a| a.name == c.name()).expect("in catalog");
            assert_eq!(listed, &c.collection);
        }
    }

    #[test]
    fn documents_never_infer_constraints_from_observed_fields() {
        let s = design_schema();
        let articles = s.collection("articles").expect("articles");
        assert_eq!(articles.heading, Some(FieldsHeading::Observed));
        assert!(
            articles
                .fields
                .iter()
                .all(|f| !f.schema.required && !f.primary)
        );
    }

    #[test]
    fn attachments_stay_on_documents() {
        let s = design_schema();
        let articles = s.collection("articles").expect("articles");
        assert!(articles.indexes.iter().any(|i| i.kind == IndexKind::Vector));
        assert_eq!(articles.profile().header_badges()[0], "Document");
    }
}
