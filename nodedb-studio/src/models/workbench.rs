//! Workbench models: result sets, plans, and the schema tree.

use serde::{Deserialize, Serialize};

use crate::models::explorer::RecordRow;
use crate::services::error::StudioError;

/// One page of query output. Pagination is the seam's responsibility: the
/// client buffers whole result sets, so the real implementation emits
/// LIMIT/OFFSET rather than holding a cursor.
///
/// Rectangular and uniquely keyed by construction: every row has exactly
/// `columns.len()` cells and a distinct `id`. Build one through
/// [`ResultSet::new`], which rejects a ragged row or a duplicate id, so a
/// decoder that drops a cell surfaces as an error instead of a grid whose
/// values sit under the wrong headers. `cells` hold display strings; sorting
/// or typed comparison needs raw values the model does not carry yet.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ResultSet {
    columns: Vec<String>,
    rows: Vec<RecordRow>,
    pub elapsed_ms: u32,
    pub scanned: String,
}

impl ResultSet {
    /// Build a result set, rejecting any row whose width differs from the
    /// header and any duplicate row id.
    pub fn new(
        columns: Vec<String>,
        rows: Vec<RecordRow>,
        elapsed_ms: u32,
        scanned: String,
    ) -> Result<Self, StudioError> {
        let width = columns.len();
        if let Some(bad) = rows.iter().find(|r| r.cells.len() != width) {
            return Err(StudioError::UnexpectedColumns {
                expected: format!("{width} cells per row"),
                got: format!("row {} has {} cells", bad.id, bad.cells.len()),
            });
        }
        // Rows are Dioxus sibling keys; a duplicate panics in debug and
        // silently reuses the wrong node in release.
        let mut seen = std::collections::HashSet::with_capacity(rows.len());
        if let Some(dup) = rows.iter().find(|r| !seen.insert(r.id.as_str())) {
            return Err(StudioError::UnexpectedColumns {
                expected: "unique row ids".into(),
                got: format!("duplicate row id {}", dup.id),
            });
        }
        Ok(Self {
            columns,
            rows,
            elapsed_ms,
            scanned,
        })
    }

    pub fn columns(&self) -> &[String] {
        &self.columns
    }

    pub fn rows(&self) -> &[RecordRow] {
        &self.rows
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, cells: &[&str]) -> RecordRow {
        RecordRow {
            id: id.into(),
            cells: cells.iter().map(|c| c.to_string()).collect(),
        }
    }

    #[test]
    fn new_accepts_rectangular_rows() {
        let rs = ResultSet::new(
            vec!["a".into(), "b".into()],
            vec![row("1", &["x", "y"]), row("2", &["p", "q"])],
            0,
            String::new(),
        );
        assert!(rs.is_ok());
    }

    #[test]
    fn new_rejects_a_short_row() {
        let rs = ResultSet::new(
            vec!["a".into(), "b".into()],
            vec![row("1", &["x", "y"]), row("2", &["only-one"])],
            0,
            String::new(),
        );
        assert!(matches!(rs, Err(StudioError::UnexpectedColumns { .. })));
    }

    #[test]
    fn new_rejects_a_duplicate_row_id() {
        let rs = ResultSet::new(
            vec!["a".into()],
            vec![row("same", &["x"]), row("same", &["y"])],
            0,
            String::new(),
        );
        assert!(matches!(rs, Err(StudioError::UnexpectedColumns { .. })));
    }

    #[test]
    fn new_rejects_a_long_row() {
        let rs = ResultSet::new(
            vec!["a".into()],
            vec![row("1", &["x", "extra"])],
            0,
            String::new(),
        );
        assert!(matches!(rs, Err(StudioError::UnexpectedColumns { .. })));
    }
}
