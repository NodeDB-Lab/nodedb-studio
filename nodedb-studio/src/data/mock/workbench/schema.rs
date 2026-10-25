//! The schema tree the Query page lists.

use crate::models::collection::Profile;
use crate::models::workbench::{SchemaField, SchemaKind, SchemaTable, SchemaTree};

fn table(
    name: &str,
    kind: SchemaKind,
    rows: Option<u64>,
    fields: &[(&str, &str, bool)],
) -> SchemaTable {
    SchemaTable {
        name: name.to_string(),
        kind,
        rows,
        fields: fields
            .iter()
            .map(|(n, t, key)| SchemaField {
                name: n.to_string(),
                type_name: t.to_string(),
                key: *key,
            })
            .collect(),
    }
}

pub fn schema_tree() -> SchemaTree {
    SchemaTree {
        tables: vec![
            table(
                "events",
                SchemaKind::Collection(Profile::Document),
                Some(2_400_000),
                &[
                    ("_id", "string", false),
                    ("type", "string", false),
                    ("user_id", "string", false),
                    ("ts", "timestamp", false),
                    ("props", "jsonb", false),
                    ("tags", "array", false),
                ],
            ),
            table(
                "users",
                SchemaKind::Collection(Profile::Document),
                Some(12_481),
                &[
                    ("_id", "string", true),
                    ("name", "string", false),
                    ("email", "string", false),
                    ("country", "char(2)", false),
                    ("created_at", "timestamp", false),
                ],
            ),
            table(
                "orders",
                SchemaKind::Collection(Profile::DocumentStrict),
                Some(442_003),
                &[
                    ("id", "int8", true),
                    ("customer_id", "string", false),
                    ("total", "decimal", false),
                    ("currency", "string", false),
                    ("status", "enum", false),
                ],
            ),
            table(
                "doc_embeddings",
                SchemaKind::Vector,
                Some(1_100_000),
                &[
                    ("id", "string", true),
                    ("embedding", "vector(768)", false),
                    ("doc_id", "string", false),
                ],
            ),
            table(
                "social_graph",
                SchemaKind::Graph,
                Some(3_200_000),
                &[
                    ("src", "string", false),
                    ("dst", "string", false),
                    ("rel", "string", false),
                ],
            ),
        ],
        views: vec![table(
            "mv_top_users_24h",
            SchemaKind::MaterializedView,
            None,
            &[("user_id", "string", false), ("events", "int8", false)],
        )],
    }
}
