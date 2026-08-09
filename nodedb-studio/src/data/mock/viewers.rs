//! Specialized-viewer fixtures: graph, vector, timeseries, spatial, FTS, sync.
//!
//! The client decodes `SubGraph` node/edge properties and `SearchResult.metadata`
//! as empty, which is why `models::viewers` defines its own display-carrying
//! types instead of reusing client types here. These fixtures are deterministic
//! stand-ins for what the real implementation will decode from raw SQL rows.

use crate::models::viewers::{
    FtsHit, GraphEdge, GraphNode, SeriesPoint, SpatialFeature, SubGraph, SyncPeer, VectorPoint,
};

/// A small connected graph for `collection`: every edge references a node
/// present in `nodes`. Ids and labels are keyed off `collection` so a caller
/// that passes the wrong collection produces visibly different data.
#[allow(dead_code)] // SEAM-UNWIRED
pub fn sub_graph(collection: &str) -> SubGraph {
    let nodes = vec![
        GraphNode {
            id: format!("{collection}-n1"),
            label: format!("{collection}-alice"),
            x: 0.0,
            y: 0.0,
        },
        GraphNode {
            id: format!("{collection}-n2"),
            label: format!("{collection}-bob"),
            x: 1.0,
            y: 0.5,
        },
        GraphNode {
            id: format!("{collection}-n3"),
            label: format!("{collection}-carol"),
            x: 2.0,
            y: 1.0,
        },
    ];
    let edges = vec![
        GraphEdge {
            id: format!("{collection}-e1"),
            from: format!("{collection}-n1"),
            to: format!("{collection}-n2"),
            label: "follows".into(),
        },
        GraphEdge {
            id: format!("{collection}-e2"),
            from: format!("{collection}-n2"),
            to: format!("{collection}-n3"),
            label: "follows".into(),
        },
    ];
    SubGraph { nodes, edges }
}

/// The genuinely-empty graph `MockBehavior::Empty` returns for `sub_graph`: a
/// collection with zero nodes is a real graph-viewer outcome, not an absent
/// value, so unlike `record_detail` this must not fold into `sub_graph`'s
/// fixture nodes.
#[allow(dead_code)] // SEAM-UNWIRED
pub fn empty_sub_graph() -> SubGraph {
    SubGraph {
        nodes: Vec::new(),
        edges: Vec::new(),
    }
}

/// A 2D projection of embeddings for `collection`, grouped into a couple of
/// clusters. Ids are keyed off `collection` so a caller that passes the
/// wrong collection produces visibly different data.
#[allow(dead_code)] // SEAM-UNWIRED
pub fn vector_points(collection: &str) -> Vec<VectorPoint> {
    (0..6)
        .map(|i| VectorPoint {
            id: format!("{collection}-vec-{i}"),
            x: i as f32 * 0.3,
            y: (i % 3) as f32 * 0.7,
            cluster: if i % 2 == 0 { "a" } else { "b" }.into(),
        })
        .collect()
}

/// Samples for one timeseries metric.
#[allow(dead_code)] // SEAM-UNWIRED
pub fn series(metric: &str) -> Vec<SeriesPoint> {
    (0..8)
        .map(|i| SeriesPoint {
            id: format!("{metric}-{i}"),
            t: format!("2026-08-08T10:0{i}:00Z"),
            value: 10.0 + i as f32,
        })
        .collect()
}

/// Spatial features for `collection`, with placeholder GeoJSON geometry. Ids
/// are keyed off `collection` so a caller that passes the wrong collection
/// produces visibly different data.
#[allow(dead_code)] // SEAM-UNWIRED
pub fn spatial_features(collection: &str) -> Vec<SpatialFeature> {
    vec![
        SpatialFeature {
            id: format!("{collection}-kl-tower"),
            name: "KL Tower".into(),
            geometry_json: r#"{"type":"Point","coordinates":[101.7038,3.1528]}"#.into(),
        },
        SpatialFeature {
            id: format!("{collection}-petronas"),
            name: "Petronas Towers".into(),
            geometry_json: r#"{"type":"Point","coordinates":[101.7119,3.1579]}"#.into(),
        },
    ]
}

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
