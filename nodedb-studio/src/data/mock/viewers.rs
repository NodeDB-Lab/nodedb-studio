//! Specialized-viewer fixtures: FTS and sync.
//!
//! The client decodes `SearchResult.metadata` as empty, which is why `models::viewers` defines its own display-carrying
//! types instead of reusing client types here. These fixtures are deterministic
//! stand-ins for what the real implementation will decode from raw SQL rows.

use crate::models::viewers::{FtsHit, SyncPeer};

/// Full-text-search hits for `query` within `collection`. Ids are keyed off
/// `collection` so a caller that passes the wrong collection produces
/// visibly different data.
#[allow(dead_code)] // SEAM-UNWIRED
pub fn fts_hits(collection: &str, query: &str) -> Vec<FtsHit> {
    (0..3)
        .map(|i| FtsHit {
            id: format!("{collection}-hit-{i}"),
            excerpt: format!("...an excerpt in {collection} mentioning {query}..."),
            score: format!("0.{}", 9 - i),
        })
        .collect()
}

/// Sync/replication peers.
#[allow(dead_code)] // SEAM-UNWIRED
pub fn sync_peers() -> Vec<SyncPeer> {
    vec![
        SyncPeer {
            id: "peer-1".into(),
            name: "eu-west".into(),
            state: "synced".into(),
            lag: "0ms".into(),
        },
        SyncPeer {
            id: "peer-2".into(),
            name: "ap-south".into(),
            state: "lagging".into(),
            lag: "820ms".into(),
        },
    ]
}
