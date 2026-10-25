//! Cluster membership and replication reads.

use async_trait::async_trait;

use crate::data::mock;
use crate::models::cluster::{ClusterMembers, HostedGroup};
use crate::services::cluster_data::ClusterData;
use crate::services::error::StudioError;
use crate::services::mock::MockConnectionService;
use crate::services::mock_behavior::{MockBehavior, apply, apply_one};

#[async_trait(?Send)]
impl ClusterData for MockConnectionService {
    async fn cluster_members(&self) -> Result<ClusterMembers, StudioError> {
        // `Empty` keeps the observation but reports no members, so the empty
        // state is reachable on a single-value read.
        let empty = self.behavior == MockBehavior::Empty;
        apply_one(self.behavior, move || {
            let mut list = mock::cluster_members();
            if empty {
                list.members.clear();
            }
            list
        })
        .await
    }

    async fn hosted_groups(&self, node_id: &str) -> Result<Vec<HostedGroup>, StudioError> {
        let id = node_id.to_string();
        apply(self.behavior, move || mock::hosted_groups(&id)).await
    }
}
