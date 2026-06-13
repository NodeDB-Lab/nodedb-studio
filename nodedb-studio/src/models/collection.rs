//! Collections and the storage modes a single NodeDB instance exposes.
//!
//! NodeDB is one engine with eight internal *storage modes*. A collection
//! belongs to exactly one mode, and Studio renders a purpose-built viewer per
//! mode (see `views/explorer/viewers/`). These are NOT separate engines or
//! databases — every collection below lives inside one NodeDB connection.

use serde::{Deserialize, Serialize};

/// The eight storage modes a NodeDB collection can use.
///
/// Ordering here is the canonical display order used by the Explorer sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorageMode {
    Document,
    Strict,
    Vector,
    Graph,
    Timeseries,
    Kv,
    Spatial,
    Fts,
}

impl StorageMode {
    /// Human-facing label as shown in the mockup's Explorer sidebar.
    pub fn label(self) -> &'static str {
        match self {
            StorageMode::Document => "Document",
            StorageMode::Strict => "Strict",
            StorageMode::Vector => "Vector",
            StorageMode::Graph => "Graph",
            StorageMode::Timeseries => "Timeseries",
            StorageMode::Kv => "KV",
            StorageMode::Spatial => "Spatial",
            StorageMode::Fts => "FTS",
        }
    }

    /// Stable machine key (used for CSS hooks / data attributes ported from
    /// the mockup, e.g. a `data-mode` value).
    pub fn key(self) -> &'static str {
        match self {
            StorageMode::Document => "document",
            StorageMode::Strict => "strict",
            StorageMode::Vector => "vector",
            StorageMode::Graph => "graph",
            StorageMode::Timeseries => "timeseries",
            StorageMode::Kv => "kv",
            StorageMode::Spatial => "spatial",
            StorageMode::Fts => "fts",
        }
    }
}

/// A single collection within a database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Collection {
    pub name: String,
    pub mode: StorageMode,
    /// Approximate item count, for the sidebar badge. Mock-only for now.
    pub count: u64,
}
