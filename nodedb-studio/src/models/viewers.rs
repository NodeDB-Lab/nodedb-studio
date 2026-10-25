//! Specialized-viewer models: FTS and sync.
//!
//! The client decodes `SearchResult.metadata` as empty, so Studio cannot get
//! display fields (excerpts) by calling those typed client methods. These
//! models carry the fields the viewers actually render; the real seam
//! implementation populates them by decoding raw SQL rows rather than the
//! client's typed search types.

use serde::{Deserialize, Serialize};

/// One full-text-search hit.
#[allow(dead_code)] // SEAM-UNWIRED
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FtsHit {
    pub id: String,
    pub excerpt: String,
    pub score: String,
}

/// One peer in the sync/replication topology.
#[allow(dead_code)] // SEAM-UNWIRED
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncPeer {
    pub id: String,
    pub name: String,
    pub state: String,
    pub lag: String,
}
