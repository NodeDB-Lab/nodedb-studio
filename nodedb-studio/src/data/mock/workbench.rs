//! Workbench fixtures: a deterministic query result, a short explain plan,
//! and a schema tree with path-like ids.

use crate::models::explorer::RecordRow;
use crate::models::workbench::{QueryPlan, ResultSet, SchemaNode};

/// A deterministic 3-column, 4-row result set. Every query text answers with
/// the same shape today; the real implementation decodes whatever the server
/// returned for `sql`.
#[allow(dead_code)] // SEAM-UNWIRED
pub fn result_set(sql: &str) -> ResultSet {
    ResultSet {
        columns: vec!["id".into(), "name".into(), "created_at".into()],
        rows: (0..4)
            .map(|i| RecordRow {
                id: format!("row-{i}"),
                cells: vec![
                    format!("row-{i}"),
                    format!("item {i}"),
                    format!("2026-08-0{} 10:0{}:00", (i % 9) + 1, i),
                ],
            })
            .collect(),
        elapsed_ms: 12,
        scanned: format!("4 rows for `{sql}`"),
    }
}

/// The genuinely-empty result set `MockBehavior::Empty` returns for
/// `run_query`: zero rows is the most common non-error query outcome, so
/// unlike `record_detail`/`explain` this must not fold into `result_set`'s
/// fixture rows. Columns are kept (a real zero-row result still has a shape)
/// so the empty state is distinguishable from an absent one.
#[allow(dead_code)] // SEAM-UNWIRED
pub fn empty_result_set(sql: &str) -> ResultSet {
    ResultSet {
        columns: vec!["id".into(), "name".into(), "created_at".into()],
        rows: Vec::new(),
        elapsed_ms: 3,
        scanned: format!("0 rows for `{sql}`"),
    }
}

/// A short, deterministic EXPLAIN plan for `sql`.
#[allow(dead_code)] // SEAM-UNWIRED
pub fn query_plan(sql: &str) -> QueryPlan {
    QueryPlan {
        text: format!("Seq Scan on users  (cost=0.00..1.04 rows=4)\n  -- {sql}"),
    }
}

/// A two-level schema tree (database -> collections -> fields) with
/// path-like ids, so uniqueness is structural rather than accidental.
#[allow(dead_code)] // SEAM-UNWIRED
pub fn schema_tree() -> Vec<SchemaNode> {
    vec![SchemaNode {
        id: "db".into(),
        label: "db".into(),
        kind: "database".into(),
        children: vec![
            SchemaNode {
                id: "db/users".into(),
                label: "users".into(),
                kind: "collection".into(),
                children: vec![
                    SchemaNode {
                        id: "db/users/id".into(),
                        label: "id".into(),
                        kind: "field".into(),
                        children: Vec::new(),
                    },
                    SchemaNode {
                        id: "db/users/name".into(),
                        label: "name".into(),
                        kind: "field".into(),
                        children: Vec::new(),
                    },
                ],
            },
            SchemaNode {
                id: "db/orders".into(),
                label: "orders".into(),
                kind: "collection".into(),
                children: vec![SchemaNode {
                    id: "db/orders/id".into(),
                    label: "id".into(),
                    kind: "field".into(),
                    children: Vec::new(),
                }],
            },
        ],
    }]
}
