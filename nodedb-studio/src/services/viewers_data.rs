//! Specialized-viewer reads at the backend seam: sync.
//! Vector Space has its own trait, `vector_data`; Timeseries has `timeseries_data`;
//! Spatial has `spatial_data`; Search has `search_data`.
//!
//! `sync_peers` is deliberately left unparameterised: it is
//! instance-scoped, not per-collection.

use async_trait::async_trait;

use crate::models::viewers::SyncPeer;
use crate::services::error::StudioError;

#[async_trait(?Send)]
pub trait ViewersData {
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
    async fn every_sync_peer_is_keyed_and_non_empty() {
        let svc = MockConnectionService::ready();
        let peers = svc.sync_peers().await.expect("peers");
        assert!(!peers.is_empty());
        assert!(
            peers.iter().all(|p| !p.id.is_empty()),
            "every sync peer needs a stable key"
        );
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
