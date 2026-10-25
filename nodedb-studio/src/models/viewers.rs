//! Specialized-viewer models: spatial, FTS, sync.
//!
//! The client decodes `SearchResult.metadata` as empty, so Studio cannot get
//! display fields (coordinates, excerpts) by calling those typed client
//! methods. These models carry the fields the viewers actually render; the
//! real seam implementation populates them by decoding raw SQL rows rather
//! than the client's typed search types.

use serde::{Deserialize, Serialize};

/// One feature in a spatial viewer. `geometry_json` is display GeoJSON
/// produced at the seam, never a raw client value.
#[allow(dead_code)] // SEAM-UNWIRED
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpatialFeature {
    pub id: String,
    pub name: String,
    pub geometry_json: String,
}

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
