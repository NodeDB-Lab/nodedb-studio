//! Workbench models: result sets, plans, and the schema tree.

use serde::{Deserialize, Serialize};

use crate::models::explorer::RecordRow;

/// One page of query output. Pagination is the seam's responsibility: the
/// client buffers whole result sets, so the real implementation emits
/// LIMIT/OFFSET rather than holding a cursor.
#[allow(dead_code)] // SEAM-UNWIRED
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultSet {
    pub columns: Vec<String>,
    pub rows: Vec<RecordRow>,
    pub elapsed_ms: u32,
    pub scanned: String,
}

/// The query planner's EXPLAIN output for one statement.
#[allow(dead_code)] // SEAM-UNWIRED
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryPlan {
    pub text: String,
}

/// One node in the schema tree (database / collection / field, recursively).
#[allow(dead_code)] // SEAM-UNWIRED
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaNode {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub children: Vec<SchemaNode>,
}
