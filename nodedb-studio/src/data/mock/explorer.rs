//! Explorer fixtures: grouped collections, list rows, and detail bodies.
//!
//! `record_detail` is exercised by `ExplorerData`'s mock/stub impls but has no
//! caller yet outside `#[cfg(test)]` (the Explorer views aren't wired to the
//! seam until a later task), so it needs `#[allow(dead_code)]` in the interim.

use crate::models::collection::{Collection, StorageMode};
use crate::models::explorer::{CollectionGroup, RecordDetail, RecordRow};

#[allow(dead_code)]
fn collection(name: &str, mode: StorageMode, count: &str) -> Collection {
    Collection {
        name: name.to_string(),
        mode,
        count: count.to_string(),
    }
}

/// Grouped in the canonical `StorageMode` display order.
#[allow(dead_code)]
pub fn collection_groups() -> Vec<CollectionGroup> {
    vec![
        CollectionGroup {
            mode: StorageMode::Document,
            collections: vec![
                collection("users", StorageMode::Document, "12,481"),
                collection("orders", StorageMode::Document, "98,204"),
            ],
        },
        CollectionGroup {
            mode: StorageMode::Strict,
            collections: vec![collection("accounts", StorageMode::Strict, "3,921")],
        },
        CollectionGroup {
            mode: StorageMode::Vector,
            collections: vec![collection("embeddings", StorageMode::Vector, "2.4M")],
        },
        CollectionGroup {
            mode: StorageMode::Graph,
            collections: vec![collection("social", StorageMode::Graph, "44,010")],
        },
        CollectionGroup {
            mode: StorageMode::Timeseries,
            collections: vec![collection("metrics", StorageMode::Timeseries, "8.1M")],
        },
        CollectionGroup {
            mode: StorageMode::Kv,
            collections: vec![collection("sessions", StorageMode::Kv, "51,003")],
        },
        CollectionGroup {
            mode: StorageMode::Spatial,
            collections: vec![collection("places", StorageMode::Spatial, "1,204")],
        },
        CollectionGroup {
            mode: StorageMode::Fts,
            collections: vec![collection("articles", StorageMode::Fts, "22,847")],
        },
    ]
}

/// List rows for a collection. Deterministic and keyed by `id`.
#[allow(dead_code)]
pub fn records(collection: &str) -> Vec<RecordRow> {
    (0..6)
        .map(|i| RecordRow {
            id: format!("{collection}-{i}"),
            cells: vec![
                format!("{collection}-{i}"),
                format!("row {i}"),
                format!("2026-08-0{} 10:0{}:00", (i % 9) + 1, i),
            ],
        })
        .collect()
}

/// Detail body for one record.
#[allow(dead_code)]
pub fn record_detail(collection: &str, id: &str) -> RecordDetail {
    RecordDetail {
        id: id.to_string(),
        title: format!("{collection} / {id}"),
        body_json: format!("{{\"id\":\"{id}\",\"collection\":\"{collection}\"}}"),
        footer: "mock fixture".to_string(),
    }
}
