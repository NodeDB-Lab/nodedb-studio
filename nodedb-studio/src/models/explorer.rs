//! Explorer-tier models: the grouped sidebar, list rows, and the detail panel.

use serde::{Deserialize, Serialize};

use crate::models::collection::{Collection, StorageMode};

/// One storage-mode group in the Explorer sidebar. `mode` is the stable key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionGroup {
    pub mode: StorageMode,
    pub collections: Vec<Collection>,
}

/// One row in a viewer's list pane. `cells` are pre-formatted for display and
/// align with the viewer's own column headers.
#[allow(dead_code)] // SEAM-UNWIRED
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordRow {
    pub id: String,
    pub cells: Vec<String>,
}

/// The detail panel for one record. `body_json` is display JSON produced at the
/// seam, never a raw client value.
#[allow(dead_code)] // SEAM-UNWIRED
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordDetail {
    pub id: String,
    pub title: String,
    pub body_json: String,
    pub footer: String,
}
