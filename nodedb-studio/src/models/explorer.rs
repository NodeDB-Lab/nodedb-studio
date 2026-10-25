//! A row of a tabular result: an id and display cells.

use serde::{Deserialize, Serialize};

/// One row in a result grid. `cells` are pre-formatted for display and
/// align with the grid's own column headers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordRow {
    pub id: String,
    pub cells: Vec<String>,
}
