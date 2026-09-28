//! Explorer-tier reads at the backend seam.
//!
//! `collection_groups` returns already-grouped collections because engine type
//! is not available in a single server call: the real implementation reads the
//! collection list, then resolves each collection's storage mode separately.
//! Keeping the grouping behind the seam means the sidebar never sees that.

use async_trait::async_trait;

use crate::models::explorer::{CollectionGroup, RecordDetail, RecordRow};
use crate::services::error::StudioError;

#[async_trait(?Send)]
pub trait ExplorerData {
    /// Sidebar contents: collections grouped by storage mode, in display order.
    async fn collection_groups(&self) -> Result<Vec<CollectionGroup>, StudioError>;

    /// List-pane rows for one collection.
    #[allow(dead_code)] // SEAM-UNWIRED
    async fn records(&self, collection: &str) -> Result<Vec<RecordRow>, StudioError>;

    /// Detail-panel contents for one record.
    #[allow(dead_code)] // SEAM-UNWIRED
    async fn record_detail(&self, collection: &str, id: &str) -> Result<RecordDetail, StudioError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::collection::StorageMode;
    use crate::services::async_state::AsyncState;
    use crate::services::connection_service::MockConnectionService;

    #[tokio::test]
    async fn groups_are_ordered_and_non_empty() {
        let svc = MockConnectionService::ready();
        let groups = svc.collection_groups().await.expect("ready yields groups");
        assert!(!groups.is_empty());
        for g in &groups {
            assert!(
                !g.collections.is_empty(),
                "an empty group would render a header with no rows"
            );
        }
        // The sidebar renders groups in this exact sequence; it must track
        // `StorageMode`'s own declared (canonical display) order.
        let expected_order = [
            StorageMode::Document,
            StorageMode::Strict,
            StorageMode::Vector,
            StorageMode::Graph,
            StorageMode::Timeseries,
            StorageMode::Kv,
            StorageMode::Spatial,
            StorageMode::Fts,
        ];
        let modes: Vec<StorageMode> = groups.iter().map(|g| g.mode).collect();
        assert_eq!(
            modes, expected_order,
            "groups must render in StorageMode's canonical display order"
        );
    }

    #[tokio::test]
    async fn every_collection_has_a_stable_unique_key() {
        let svc = MockConnectionService::ready();
        let groups = svc.collection_groups().await.expect("ready yields groups");
        let mut names: Vec<&str> = groups
            .iter()
            .flat_map(|g| g.collections.iter().map(|c| c.name.as_str()))
            .collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(total, names.len(), "collection names must be unique keys");
    }

    #[tokio::test]
    async fn records_have_unique_ids() {
        let svc = MockConnectionService::ready();
        let rows = svc.records("users").await.expect("ready yields rows");
        let mut ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(total, ids.len());
    }

    #[tokio::test]
    async fn empty_behaviour_reaches_the_empty_state() {
        let svc = MockConnectionService::empty();
        let s = AsyncState::from_value(Some(svc.collection_groups().await));
        assert!(s.is_empty());
    }

    #[tokio::test]
    async fn erroring_behaviour_reaches_the_error_state() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.collection_groups().await));
        assert!(s.error_message().is_some());
    }

    #[tokio::test]
    async fn record_detail_ready_returns_the_requested_id() {
        let svc = MockConnectionService::ready();
        let detail = svc
            .record_detail("users", "u1")
            .await
            .expect("ready yields a detail");
        assert_eq!(detail.id, "u1");
    }

    #[tokio::test]
    async fn record_detail_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        assert!(svc.record_detail("users", "u1").await.is_err());
    }

    #[tokio::test]
    async fn record_detail_empty_still_returns_the_requested_record() {
        // `record_detail` reads a single record, not a list: "no rows" has no
        // meaning here, so the mock folds `MockBehavior::Empty` into the same
        // success path as `Ready` rather than inventing an absent/empty detail.
        // Pinned here so a later refactor cannot silently change that meaning.
        let svc = MockConnectionService::empty();
        let detail = svc
            .record_detail("users", "u1")
            .await
            .expect("empty behaviour still returns a detail for a single-value read");
        assert_eq!(detail.id, "u1");
    }
}
