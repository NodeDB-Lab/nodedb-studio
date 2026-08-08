//! Specialized-viewer reads at the backend seam: graph, vector, timeseries,
//! spatial, FTS, and sync.
//!
//! `sub_graph` returns a single `SubGraph` rather than a list: unlike the
//! other five reads it cannot be expressed as `apply(self.behavior, ...)`,
//! which only knows how to fold `MockBehavior::Empty` into `Vec::new()`. It
//! is implemented with the same explicit four-arm match `record_detail` and
//! `run_query` use.

use async_trait::async_trait;

use crate::models::viewers::{
    FtsHit, SeriesPoint, SpatialFeature, SubGraph, SyncPeer, VectorPoint,
};
use crate::services::error::StudioError;

#[async_trait(?Send)]
pub trait ViewersData {
    /// One graph viewer's full render input (nodes + edges).
    #[allow(dead_code)] // SEAM-UNWIRED(task-10)
    async fn sub_graph(&self) -> Result<SubGraph, StudioError>;

    /// A 2D projection of vector embeddings for the vector viewer.
    #[allow(dead_code)] // SEAM-UNWIRED(task-10)
    async fn vector_points(&self) -> Result<Vec<VectorPoint>, StudioError>;

    /// Samples for one timeseries metric.
    #[allow(dead_code)] // SEAM-UNWIRED(task-10)
    async fn series(&self, metric: &str) -> Result<Vec<SeriesPoint>, StudioError>;

    /// Features for the spatial viewer's map.
    #[allow(dead_code)] // SEAM-UNWIRED(task-10)
    async fn spatial_features(&self) -> Result<Vec<SpatialFeature>, StudioError>;

    /// Full-text-search hits for `query`.
    #[allow(dead_code)] // SEAM-UNWIRED(task-10)
    async fn fts_hits(&self, query: &str) -> Result<Vec<FtsHit>, StudioError>;

    /// Sync/replication peers.
    #[allow(dead_code)] // SEAM-UNWIRED(task-10)
    async fn sync_peers(&self) -> Result<Vec<SyncPeer>, StudioError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::async_state::AsyncState;
    use crate::services::connection_service::MockConnectionService;

    #[tokio::test]
    async fn subgraph_edges_reference_existing_nodes() {
        let svc = MockConnectionService::ready();
        let g = svc.sub_graph().await.expect("graph");
        let ids: Vec<&str> = g.nodes.iter().map(|n| n.id.as_str()).collect();
        assert!(!g.nodes.is_empty() && !g.edges.is_empty());
        for e in &g.edges {
            assert!(
                ids.contains(&e.from.as_str()),
                "dangling edge from {}",
                e.from
            );
            assert!(ids.contains(&e.to.as_str()), "dangling edge to {}", e.to);
        }
    }

    #[tokio::test]
    async fn every_viewer_read_is_keyed_and_non_empty() {
        let svc = MockConnectionService::ready();

        let vector = svc.vector_points().await.expect("vec");
        assert!(!vector.is_empty());
        assert!(
            vector.iter().all(|p| !p.id.is_empty()),
            "every vector point needs a stable key"
        );

        let series = svc.series("qps").await.expect("series");
        assert!(!series.is_empty());
        assert!(
            series.iter().all(|p| !p.id.is_empty()),
            "every series point needs a stable key"
        );

        let spatial = svc.spatial_features().await.expect("geo");
        assert!(!spatial.is_empty());
        assert!(
            spatial.iter().all(|f| !f.id.is_empty()),
            "every spatial feature needs a stable key"
        );

        let fts = svc.fts_hits("nodedb").await.expect("fts");
        assert!(!fts.is_empty());
        assert!(
            fts.iter().all(|h| !h.id.is_empty()),
            "every fts hit needs a stable key"
        );

        let peers = svc.sync_peers().await.expect("peers");
        assert!(!peers.is_empty());
        assert!(
            peers.iter().all(|p| !p.id.is_empty()),
            "every sync peer needs a stable key"
        );
    }

    // Each method below gets its own named ready/empty/erroring coverage:
    // `apply(self.behavior, mock::x)` and a mis-wired `Ok(mock::x())` both
    // satisfy a single shared assertion, so every method needs its own proof
    // that it actually reads `self.behavior` (see admin_data.rs).

    #[tokio::test]
    async fn sub_graph_ready_returns_nodes_and_edges() {
        let svc = MockConnectionService::ready();
        let g = svc.sub_graph().await.expect("ready yields a graph");
        assert!(!g.nodes.is_empty(), "fixture must have nodes");
        assert!(!g.edges.is_empty(), "fixture must have edges");
    }

    #[tokio::test]
    async fn sub_graph_empty_behaviour_still_returns_a_graph() {
        // `sub_graph` reads a single value, not a list: "no rows" has no
        // meaning here, so the mock folds `MockBehavior::Empty` into the same
        // success path as `Ready`, mirroring `record_detail` and `run_query`.
        // Pinned here so a later refactor cannot silently change that meaning.
        let svc = MockConnectionService::empty();
        let g = svc
            .sub_graph()
            .await
            .expect("empty behaviour still returns a graph for a single-value read");
        assert!(
            !g.nodes.is_empty(),
            "folded-Empty graph must still have nodes"
        );
    }

    #[tokio::test]
    async fn sub_graph_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        assert!(svc.sub_graph().await.is_err());
    }

    #[tokio::test]
    async fn vector_points_ready_has_two_clusters() {
        // Expectation authored independently of the fixture body: the fixture
        // alternates clusters "a"/"b", so both must be present.
        let svc = MockConnectionService::ready();
        let pts = svc.vector_points().await.expect("ready yields points");
        assert!(pts.iter().any(|p| p.cluster == "a"));
        assert!(pts.iter().any(|p| p.cluster == "b"));
    }

    #[tokio::test]
    async fn vector_points_empty_is_empty() {
        let svc = MockConnectionService::empty();
        let s = AsyncState::from_value(Some(svc.vector_points().await));
        assert!(s.is_empty());
    }

    #[tokio::test]
    async fn vector_points_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.vector_points().await));
        assert!(s.error_message().is_some());
    }

    #[tokio::test]
    async fn series_ready_ids_are_prefixed_with_the_requested_metric() {
        let svc = MockConnectionService::ready();
        let points = svc.series("qps").await.expect("ready yields points");
        assert!(
            points.iter().all(|p| p.id.starts_with("qps-")),
            "series ids must key off the requested metric, not a fixed name"
        );
    }

    #[tokio::test]
    async fn series_empty_is_empty() {
        let svc = MockConnectionService::empty();
        let s = AsyncState::from_value(Some(svc.series("qps").await));
        assert!(s.is_empty());
    }

    #[tokio::test]
    async fn series_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.series("qps").await));
        assert!(s.error_message().is_some());
    }

    #[tokio::test]
    async fn spatial_features_ready_have_geometry() {
        let svc = MockConnectionService::ready();
        let feats = svc.spatial_features().await.expect("ready yields features");
        assert!(
            feats.iter().all(|f| !f.geometry_json.is_empty()),
            "every feature must carry display geometry"
        );
    }

    #[tokio::test]
    async fn spatial_features_empty_is_empty() {
        let svc = MockConnectionService::empty();
        let s = AsyncState::from_value(Some(svc.spatial_features().await));
        assert!(s.is_empty());
    }

    #[tokio::test]
    async fn spatial_features_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.spatial_features().await));
        assert!(s.error_message().is_some());
    }

    #[tokio::test]
    async fn fts_hits_ready_excerpts_mention_the_query() {
        let svc = MockConnectionService::ready();
        let hits = svc.fts_hits("nodedb").await.expect("ready yields hits");
        assert!(
            hits.iter().all(|h| h.excerpt.contains("nodedb")),
            "excerpts must reflect the requested query, not a fixed string"
        );
    }

    #[tokio::test]
    async fn fts_hits_empty_is_empty() {
        let svc = MockConnectionService::empty();
        let s = AsyncState::from_value(Some(svc.fts_hits("nodedb").await));
        assert!(s.is_empty());
    }

    #[tokio::test]
    async fn fts_hits_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.fts_hits("nodedb").await));
        assert!(s.error_message().is_some());
    }

    #[tokio::test]
    async fn sync_peers_ready_has_a_lagging_and_a_synced_peer() {
        let svc = MockConnectionService::ready();
        let peers = svc.sync_peers().await.expect("ready yields peers");
        assert!(peers.iter().any(|p| p.state == "synced"));
        assert!(peers.iter().any(|p| p.state == "lagging"));
    }

    #[tokio::test]
    async fn sync_peers_empty_is_empty() {
        let svc = MockConnectionService::empty();
        let s = AsyncState::from_value(Some(svc.sync_peers().await));
        assert!(s.is_empty());
    }

    #[tokio::test]
    async fn sync_peers_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.sync_peers().await));
        assert!(s.error_message().is_some());
    }
}
