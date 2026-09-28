//! Specialized-viewer models: graph, vector, timeseries, spatial, FTS, sync.
//!
//! The client decodes `SubGraph` node/edge properties and `SearchResult.metadata`
//! as empty, so Studio cannot get display fields (labels, coordinates, excerpts)
//! by calling those typed client methods. These models carry the fields the
//! viewers actually render; the real seam implementation populates them by
//! decoding raw SQL rows rather than the client's typed graph/search types.

use serde::{Deserialize, Serialize};

/// One node in a graph viewer. `x`/`y` are a laid-out display position, not
/// stored coordinates.
#[allow(dead_code)] // SEAM-UNWIRED
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub x: f32,
    pub y: f32,
}

/// One edge in a graph viewer. `from`/`to` reference `GraphNode::id`.
#[allow(dead_code)] // SEAM-UNWIRED
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub label: String,
}

/// A graph viewer's full render input: every edge must reference a node
/// present in `nodes`.
#[allow(dead_code)] // SEAM-UNWIRED
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// One point in a vector viewer's projection. `x`/`y` are a 2D projection of
/// the embedding, not the raw vector.
#[allow(dead_code)] // SEAM-UNWIRED
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorPoint {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub cluster: String,
}

/// One sample in a timeseries metric.
#[allow(dead_code)] // SEAM-UNWIRED
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeriesPoint {
    pub id: String,
    pub t: String,
    pub value: f32,
}

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
