//! Workbench fixtures: a deterministic query result, a short explain plan,
//! and a schema tree with path-like ids.

use crate::models::explorer::RecordRow;
use crate::models::workbench::{QueryPlan, ResultSet, SchemaNode};

/// A deterministic 3-column, 4-row result set. Every query text answers with
/// the same shape today; the real implementation decodes whatever the server
/// returned for `sql`.
#[allow(dead_code)] // SEAM-UNWIRED
pub fn result_set(sql: &str) -> ResultSet {
    let rows = (0..4)
        .map(|i| RecordRow {
            id: format!("row-{i}"),
            cells: vec![
                format!("row-{i}"),
                format!("item {i}"),
                format!("2026-08-0{} 10:0{}:00", (i % 9) + 1, i),
            ],
        })
        .collect();
    // The fallback keeps this infallible without an unwrap. It would swap a
    // ragged fixture for a zero-row one, so `workbench_data::run_query_ready_*`
    // pins 3 columns and 4 rows with independent numbers; a ragged edit
    // fails there rather than reaching a screen as "no rows".
    ResultSet::new(
        vec!["id".into(), "name".into(), "created_at".into()],
        rows,
        12,
        format!("4 rows for `{sql}`"),
    )
    .unwrap_or_else(|_| empty_result_set(sql))
}

/// The genuinely-empty result set `MockBehavior::Empty` returns for
/// `run_query`: zero rows is the most common non-error query outcome, so
/// unlike `record_detail`/`explain` this must not fold into `result_set`'s
/// fixture rows. Columns are shape documentation for the fixture; a zero-row
/// render with headers would need a payload-carrying Empty variant, which
/// does not exist in the current `AsyncState` design.
#[allow(dead_code)] // SEAM-UNWIRED
pub fn empty_result_set(sql: &str) -> ResultSet {
    // Zero rows are trivially rectangular, so `new` cannot fail here; the
    // fallback exists only to keep this infallible without an unwrap.
    ResultSet::new(
        vec!["id".into(), "name".into(), "created_at".into()],
        Vec::new(),
        3,
        format!("0 rows for `{sql}`"),
    )
    .unwrap_or_else(|_| ResultSet::default())
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
