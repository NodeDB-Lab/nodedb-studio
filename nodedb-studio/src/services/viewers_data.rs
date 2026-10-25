//! Specialized-viewer reads at the backend seam: spatial, FTS, and sync.
//! Vector Space has its own trait, `vector_data`; Timeseries has `timeseries_data`.
//!
//! `spatial_features` and `fts_hits` take a `collection`: the
//! Explorer already scopes its selection to one collection per storage mode,
//! so each viewer's read is parameterised the same way `records(collection)`
//! is. `sync_peers` is deliberately left unparameterised: it is
//! instance-scoped, not per-collection.

use async_trait::async_trait;

use crate::models::viewers::{FtsHit, SpatialFeature, SyncPeer};
use crate::services::error::StudioError;

#[async_trait(?Send)]
pub trait ViewersData {
    /// Features for the spatial viewer's map, scoped to `collection`.
    #[allow(dead_code)] // SEAM-UNWIRED
    async fn spatial_features(&self, collection: &str) -> Result<Vec<SpatialFeature>, StudioError>;

    /// Full-text-search hits for `query` within `collection`.
    #[allow(dead_code)] // SEAM-UNWIRED
    async fn fts_hits(&self, collection: &str, query: &str) -> Result<Vec<FtsHit>, StudioError>;

    /// Sync/replication peers.
    #[allow(dead_code)] // SEAM-UNWIRED
    async fn sync_peers(&self) -> Result<Vec<SyncPeer>, StudioError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::async_state::AsyncState;
    use crate::services::mock::MockConnectionService;

    #[tokio::test]
    async fn every_viewer_read_is_keyed_and_non_empty() {
        let svc = MockConnectionService::ready();

        let spatial = svc.spatial_features("places").await.expect("geo");
        assert!(!spatial.is_empty());
        assert!(
            spatial.iter().all(|f| !f.id.is_empty()),
            "every spatial feature needs a stable key"
        );

        let fts = svc.fts_hits("articles", "nodedb").await.expect("fts");
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
    async fn spatial_features_ready_have_geometry() {
        let svc = MockConnectionService::ready();
        let feats = svc
            .spatial_features("places")
            .await
            .expect("ready yields features");
        assert!(
            feats.iter().all(|f| !f.geometry_json.is_empty()),
            "every feature must carry display geometry"
        );
    }

    #[tokio::test]
    async fn spatial_features_ids_vary_by_collection() {
        let svc = MockConnectionService::ready();
        let a = svc.spatial_features("places").await.expect("geo");
        let b = svc.spatial_features("orders").await.expect("geo");
        assert_ne!(
            a.first().map(|f| f.id.as_str()),
            b.first().map(|f| f.id.as_str()),
            "spatial feature ids must key off the requested collection"
        );
    }

    #[tokio::test]
    async fn spatial_features_empty_is_empty() {
        let svc = MockConnectionService::empty();
        let s = AsyncState::from_value(Some(svc.spatial_features("places").await));
        assert!(s.is_empty());
    }

    #[tokio::test]
    async fn spatial_features_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.spatial_features("places").await));
        assert!(s.error_message().is_some());
    }

    #[tokio::test]
    async fn fts_hits_ready_excerpts_mention_the_query() {
        let svc = MockConnectionService::ready();
        let hits = svc
            .fts_hits("articles", "nodedb")
            .await
            .expect("ready yields hits");
        assert!(
            hits.iter().all(|h| h.excerpt.contains("nodedb")),
            "excerpts must reflect the requested query, not a fixed string"
        );
    }

    #[tokio::test]
    async fn fts_hits_ids_vary_by_collection() {
        let svc = MockConnectionService::ready();
        let a = svc.fts_hits("articles", "nodedb").await.expect("fts");
        let b = svc.fts_hits("orders", "nodedb").await.expect("fts");
        assert_ne!(
            a.first().map(|h| h.id.as_str()),
            b.first().map(|h| h.id.as_str()),
            "fts hit ids must key off the requested collection"
        );
    }

    #[tokio::test]
    async fn fts_hits_empty_is_empty() {
        let svc = MockConnectionService::empty();
        let s = AsyncState::from_value(Some(svc.fts_hits("articles", "nodedb").await));
        assert!(s.is_empty());
    }

    #[tokio::test]
    async fn fts_hits_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.fts_hits("articles", "nodedb").await));
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
