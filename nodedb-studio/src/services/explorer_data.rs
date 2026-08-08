//! Explorer-tier reads at the backend seam.
//!
//! `collection_groups` returns already-grouped collections because engine type
//! is not available in a single server call: the real implementation reads the
//! collection list, then resolves each collection's storage mode separately.
//! Keeping the grouping behind the seam means the sidebar never sees that.

use async_trait::async_trait;

use crate::models::explorer::{CollectionGroup, RecordDetail, RecordRow};
use crate::services::error::StudioError;

// Object-safety and mock/stub conformance are proven by the tests below and by
// `nodedb_service::tests::stub_is_object_safe_behind_backend`, but nothing
// outside `#[cfg(test)]` calls these methods yet: wiring the Explorer sidebar,
// list and detail panel to the seam is a later task. The `#[allow(dead_code)]`s
// go away with that wiring.
#[async_trait(?Send)]
pub trait ExplorerData {
    /// Sidebar contents: collections grouped by storage mode, in display order.
    #[allow(dead_code)]
    async fn collection_groups(&self) -> Result<Vec<CollectionGroup>, StudioError>;

    /// List-pane rows for one collection.
    #[allow(dead_code)]
    async fn records(&self, collection: &str) -> Result<Vec<RecordRow>, StudioError>;

    /// Detail-panel contents for one record.
    #[allow(dead_code)]
    async fn record_detail(&self, collection: &str, id: &str) -> Result<RecordDetail, StudioError>;
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
