use crate::models::schema::{
    CellFormat, CollectionSchema, ColumnSpec, FieldType, IndexDef, IndexKind, SchemaField,
    TextIndex, VectorField,
};

fn field(name: &str, ty: FieldType, required: bool) -> SchemaField {
    SchemaField {
        name: name.to_string(),
        ty,
        required,
        reference: None,
        time_key: false,
    }
}

fn text(max: Option<u32>) -> FieldType {
    FieldType::String { max }
}

fn col(header: &str, field: &str, format: CellFormat) -> ColumnSpec {
    ColumnSpec {
        header: header.to_string(),
        field: field.to_string(),
        format,
    }
}

fn index(name: &str, kind: IndexKind, fields: &[&str]) -> IndexDef {
    IndexDef {
        name: name.to_string(),
        kind,
        fields: fields.iter().map(|f| f.to_string()).collect(),
        ready: true,
    }
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|n| n.to_string()).collect()
}

fn base(collection: &str) -> CollectionSchema {
    CollectionSchema::new(collection)
}

fn users() -> CollectionSchema {
    CollectionSchema {
        id_field: Some("id".into()),
        fields: vec![
            field("id", text(None), true),
            field("name", text(None), true),
            field("email", text(None), true),
            field("status", text(None), true),
            field("role", text(None), false),
            field("created_at", FieldType::Timestamp, false),
            field("updated_at", FieldType::Timestamp, true),
            field("profile", FieldType::Object, false),
        ],
        columns: vec![
            col("id", "id", CellFormat::Id),
            col("name", "name", CellFormat::Text),
            col("email", "email", CellFormat::Text),
            col("status", "status", CellFormat::StatusDot),
            col("updated_at", "updated_at", CellFormat::Timestamp),
        ],
        indexes: vec![
            index("users_pkey", IndexKind::Primary, &["id"]),
            index("idx_users_email", IndexKind::Secondary, &["email"]),
        ],
        default_page_size: 5,
        default_sort: Some(("updated_at".into(), true)),
        filter_fields: names(&["status", "role", "name", "email"]),
        graph_binding: Some("social_graph".into()),
        ..base("users")
    }
}

fn orders() -> CollectionSchema {
    let mut customer = field("customer_id", text(None), true);
    customer.reference = Some("customers".into());
    CollectionSchema {
        id_field: Some("order_id".into()),
        fields: vec![
            field("order_id", text(None), true),
            customer,
            field(
                "total",
                FieldType::Decimal {
                    precision: 10,
                    scale: 2,
                },
                true,
            ),
            field("currency", text(Some(3)), true),
            field("status", text(None), true),
            field("created_at", FieldType::Datetime, true),
            field("items", FieldType::Array, false),
            field("shipping_address", FieldType::Object, false),
            field("metadata", FieldType::Object, false),
        ],
        columns: vec![
            col("order_id", "order_id", CellFormat::Id),
            col("customer_id", "customer_id", CellFormat::Text),
            col("total", "total", CellFormat::Number),
            col("currency", "currency", CellFormat::Text),
            col("status", "status", CellFormat::StatusBadge),
            col("created_at", "created_at", CellFormat::Timestamp),
        ],
        indexes: vec![
            index("orders_pkey", IndexKind::Primary, &["order_id"]),
            index(
                "idx_orders_customer",
                IndexKind::Secondary,
                &["customer_id"],
            ),
        ],
        default_page_size: 50,
        default_sort: Some(("created_at".into(), true)),
        filter_fields: names(&["currency", "status", "customer_id"]),
        ..base("orders")
    }
}

fn sales() -> CollectionSchema {
    let mut sold_at = field("sold_at", FieldType::Timestamp, true);
    sold_at.time_key = true;
    CollectionSchema {
        fields: vec![
            sold_at,
            field("store_id", text(None), true),
            field("product_id", text(None), true),
            field("quantity", FieldType::Int64, true),
            field(
                "revenue",
                FieldType::Decimal {
                    precision: 12,
                    scale: 2,
                },
                true,
            ),
            field("currency", text(Some(3)), true),
        ],
        columns: vec![
            col("sold_at", "sold_at", CellFormat::Timestamp),
            col("store_id", "store_id", CellFormat::Text),
            col("product_id", "product_id", CellFormat::Id),
            col("quantity", "quantity", CellFormat::Number),
            col("revenue", "revenue", CellFormat::Number),
            col("currency", "currency", CellFormat::Text),
        ],
        indexes: vec![index("sales_time", IndexKind::Primary, &["sold_at"])],
        default_page_size: 10,
        default_sort: Some(("sold_at".into(), true)),
        filter_fields: names(&["store_id", "product_id", "currency"]),
        ..base("sales")
    }
}

fn places() -> CollectionSchema {
    CollectionSchema {
        id_field: Some("feature_id".into()),
        fields: vec![
            field("feature_id", text(None), true),
            field("name", text(None), true),
            field("category", text(None), false),
            field("geometry", FieldType::Geometry { srid: 4326 }, true),
        ],
        columns: vec![
            col("feature_id", "feature_id", CellFormat::Text),
            col("name", "name", CellFormat::Id),
            col("category", "category", CellFormat::Text),
            col("geometry", "geometry", CellFormat::Geometry),
            col("SRID", "", CellFormat::Srid),
        ],
        indexes: vec![
            index("places_pkey", IndexKind::Primary, &["feature_id"]),
            index("idx_places_geometry", IndexKind::Spatial, &["geometry"]),
        ],
        default_page_size: 10,
        default_sort: None,
        filter_fields: names(&["category", "name"]),
        ..base("places")
    }
}

fn articles() -> CollectionSchema {
    CollectionSchema {
        id_field: Some("_id".into()),
        fields: vec![
            field("_id", text(None), true),
            field("title", text(None), true),
            field("body", text(None), false),
            field("language", text(Some(2)), false),
            field("updated_at", FieldType::Timestamp, false),
            field("embedding", FieldType::Vector { dimensions: 768 }, false),
            field("model", text(None), false),
            field("embedded_at", FieldType::Timestamp, false),
        ],
        columns: vec![
            col("_id", "_id", CellFormat::Id),
            col("title", "title", CellFormat::Text),
            col("language", "language", CellFormat::Text),
            col("embedding", "embedding", CellFormat::Embedding),
            col("model", "model", CellFormat::Text),
            col("embedded_at", "embedded_at", CellFormat::Timestamp),
        ],
        indexes: vec![
            index("articles_pkey", IndexKind::Primary, &["_id"]),
            index("idx_articles_text", IndexKind::FullText, &["title", "body"]),
            index("idx_articles_embedding", IndexKind::Vector, &["embedding"]),
        ],
        default_page_size: 10,
        default_sort: Some(("_id".into(), true)),
        filter_fields: names(&["language", "model"]),
        text_index: Some(TextIndex {
            name: "idx_articles_text".into(),
            fields: vec![
                ("title".into(), "English".into()),
                ("body".into(), "English".into()),
            ],
            ready: true,
            coverage_percent: None,
        }),
        vector: Some(VectorField {
            field: "embedding".into(),
            dimensions: 768,
            model: "support-embed-v2".into(),
            model_revision: 2,
            normalized: true,
            index: "idx_articles_embedding".into(),
        }),
        ..base("articles")
    }
}

fn sessions() -> CollectionSchema {
    CollectionSchema {
        id_field: Some("key".into()),
        fields: vec![
            field("key", text(None), true),
            field("type", text(None), true),
            field("size", FieldType::Int64, true),
            field("ttl", FieldType::Int64, false),
            field("value", FieldType::Json, true),
        ],
        columns: vec![
            col("key", "key", CellFormat::Id),
            col("type", "type", CellFormat::Text),
            col("size", "size", CellFormat::Bytes),
            col("ttl", "ttl", CellFormat::Ttl),
            col("preview", "value", CellFormat::Preview),
        ],
        indexes: vec![index("sessions_key", IndexKind::Primary, &["key"])],
        default_page_size: 20,
        default_sort: None,
        filter_fields: names(&["type"]),
        ..base("sessions")
    }
}

fn inventory() -> CollectionSchema {
    CollectionSchema {
        fields: vec![
            field("sku", text(None), true),
            field("warehouse", text(None), true),
            field("stock", FieldType::Int64, true),
            field(
                "price",
                FieldType::Decimal {
                    precision: 10,
                    scale: 2,
                },
                true,
            ),
            field("updated_at", FieldType::Timestamp, true),
        ],
        columns: vec![
            col("sku", "sku", CellFormat::Text),
            col("warehouse", "warehouse", CellFormat::Text),
            col("stock", "stock", CellFormat::Number),
            col("price", "price", CellFormat::Number),
            col("updated_at", "updated_at", CellFormat::Timestamp),
        ],
        indexes: vec![index(
            "inventory_sku",
            IndexKind::Primary,
            &["sku", "warehouse"],
        )],
        default_page_size: 10,
        default_sort: Some(("sku".into(), false)),
        filter_fields: names(&["warehouse", "sku"]),
        typed_headers: true,
        ..base("inventory")
    }
}

fn shipments() -> CollectionSchema {
    CollectionSchema {
        id_field: Some("_id".into()),
        fields: vec![
            field("_id", text(None), true),
            field("order_id", text(None), true),
            field("status", text(None), true),
            field("origin", text(None), true),
            field("destination", text(None), true),
            field("scheduled_at", FieldType::Timestamp, true),
            field("updated_at", FieldType::Timestamp, true),
        ],
        columns: vec![
            col("_id", "_id", CellFormat::Text),
            col("order_id", "order_id", CellFormat::Text),
            col("status", "status", CellFormat::Text),
            col("origin", "origin", CellFormat::Text),
            col("destination", "destination", CellFormat::Text),
            col("scheduled_at", "scheduled_at", CellFormat::Timestamp),
            col("updated_at", "updated_at", CellFormat::Timestamp),
        ],
        indexes: vec![index("shipments_pkey", IndexKind::Primary, &["_id"])],
        default_page_size: 50,
        default_sort: Some(("updated_at".into(), true)),
        filter_fields: names(&["status", "origin", "destination"]),
        ..base("shipments")
    }
}

/// The schema of a record collection. `None` for a name the catalog does
/// not hold, and for the array, which has no record schema.
pub fn collection_schema(collection: &str) -> Option<CollectionSchema> {
    Some(match collection {
        "users" => users(),
        "orders" => orders(),
        "sales" => sales(),
        "places" => places(),
        "articles" => articles(),
        "sessions" => sessions(),
        "inventory" => inventory(),
        "shipments" => shipments(),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::mock::explorer::catalog::collections;
    use crate::models::collection::Profile;

    #[test]
    fn every_record_collection_has_a_schema_whose_columns_name_real_fields() {
        for c in collections() {
            if c.profile == Profile::Array {
                assert!(collection_schema(&c.name).is_none());
                continue;
            }
            let schema = collection_schema(&c.name).expect("schema");
            assert!(!schema.columns.is_empty(), "{}", c.name);
            for column in &schema.columns {
                assert!(
                    column.field.is_empty() || schema.field(&column.field).is_some(),
                    "{}: column `{}` reads a missing field",
                    c.name,
                    column.header
                );
            }
            for f in &schema.filter_fields {
                assert!(schema.field(f).is_some(), "{}: filter on `{f}`", c.name);
            }
        }
    }

    #[test]
    fn only_timeseries_declares_a_time_key() {
        for c in collections() {
            let has = collection_schema(&c.name).is_some_and(|s| s.time_key().is_some());
            assert_eq!(has, c.profile == Profile::Timeseries, "{}", c.name);
        }
    }

    #[test]
    fn capabilities_match_the_schema_facts() {
        use crate::models::collection::Attached;
        for c in collections() {
            let Some(s) = collection_schema(&c.name) else {
                continue;
            };
            assert_eq!(
                s.graph_binding.is_some(),
                c.has(Attached::Graph),
                "{}",
                c.name
            );
            assert_eq!(
                s.text_index.is_some(),
                c.has(Attached::FullText),
                "{}",
                c.name
            );
            assert_eq!(s.vector.is_some(), c.has(Attached::Vector), "{}", c.name);
        }
    }
}
