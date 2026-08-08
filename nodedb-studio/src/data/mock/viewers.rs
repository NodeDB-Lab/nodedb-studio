//! Specialized-viewer fixtures: graph, vector, timeseries, spatial, FTS, sync.
//!
//! The client decodes `SubGraph` node/edge properties and `SearchResult.metadata`
//! as empty, which is why `models::viewers` defines its own display-carrying
//! types instead of reusing client types here. These fixtures are deterministic
//! stand-ins for what the real implementation will decode from raw SQL rows.

use crate::models::viewers::{
    FtsHit, GraphEdge, GraphNode, SeriesPoint, SpatialFeature, SubGraph, SyncPeer, VectorPoint,
};

/// A small connected graph: every edge references a node present in `nodes`.
#[allow(dead_code)] // SEAM-UNWIRED(task-10)
pub fn sub_graph() -> SubGraph {
    let nodes = vec![
        GraphNode {
            id: "n1".into(),
            label: "alice".into(),
            x: 0.0,
            y: 0.0,
        },
        GraphNode {
            id: "n2".into(),
            label: "bob".into(),
            x: 1.0,
            y: 0.5,
        },
        GraphNode {
            id: "n3".into(),
            label: "carol".into(),
            x: 2.0,
            y: 1.0,
        },
    ];
    let edges = vec![
        GraphEdge {
            id: "e1".into(),
            from: "n1".into(),
            to: "n2".into(),
            label: "follows".into(),
        },
        GraphEdge {
            id: "e2".into(),
            from: "n2".into(),
            to: "n3".into(),
            label: "follows".into(),
        },
    ];
    SubGraph { nodes, edges }
}

/// A 2D projection of embeddings, grouped into a couple of clusters.
#[allow(dead_code)] // SEAM-UNWIRED(task-10)
pub fn vector_points() -> Vec<VectorPoint> {
    (0..6)
        .map(|i| VectorPoint {
            id: format!("vec-{i}"),
            x: i as f32 * 0.3,
            y: (i % 3) as f32 * 0.7,
            cluster: if i % 2 == 0 { "a" } else { "b" }.into(),
        })
        .collect()
}

/// Samples for one timeseries metric.
#[allow(dead_code)] // SEAM-UNWIRED(task-10)
pub fn series(metric: &str) -> Vec<SeriesPoint> {
    (0..8)
        .map(|i| SeriesPoint {
            id: format!("{metric}-{i}"),
            t: format!("2026-08-08T10:0{i}:00Z"),
            value: 10.0 + i as f32,
        })
        .collect()
}

/// Spatial features with placeholder GeoJSON geometry.
#[allow(dead_code)] // SEAM-UNWIRED(task-10)
pub fn spatial_features() -> Vec<SpatialFeature> {
    vec![
        SpatialFeature {
            id: "kl-tower".into(),
            name: "KL Tower".into(),
            geometry_json: r#"{"type":"Point","coordinates":[101.7038,3.1528]}"#.into(),
        },
        SpatialFeature {
            id: "petronas".into(),
            name: "Petronas Towers".into(),
            geometry_json: r#"{"type":"Point","coordinates":[101.7119,3.1579]}"#.into(),
        },
    ]
}

/// Full-text-search hits for `query`.
#[allow(dead_code)] // SEAM-UNWIRED(task-10)
pub fn fts_hits(query: &str) -> Vec<FtsHit> {
    (0..3)
        .map(|i| FtsHit {
            id: format!("hit-{i}"),
            excerpt: format!("...an excerpt mentioning {query}..."),
            score: format!("0.{}", 9 - i),
        })
        .collect()
}

/// Sync/replication peers.
#[allow(dead_code)] // SEAM-UNWIRED(task-10)
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
