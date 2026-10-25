//! Cluster reads and operations.

use async_trait::async_trait;

use crate::data::mock;
use crate::models::cluster::{ClusterMembers, HostedGroup};
use crate::models::cluster_activity::{ActivityLog, ActivityQuery};
use crate::models::cluster_monitor::{Telemetry, TelemetryRange};
use crate::models::cluster_op::OpResult;
use crate::models::cluster_place::{MoveRequest, Placement, RebalancePlan};
use crate::models::cluster_raft::{RaftGroups, SnapshotInfo, TransferPreview};
use crate::services::cluster_data::ClusterData;
use crate::services::error::StudioError;
use crate::services::mock::MockConnectionService;
use crate::services::mock_behavior::{
    MockBehavior, apply, apply_one, apply_one_result, apply_write,
};

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
        let hosted = self.cluster.borrow().hosted(node_id);
        apply(self.behavior, move || hosted).await
    }

    async fn raft_groups(&self) -> Result<RaftGroups, StudioError> {
        let mut read = self.cluster.borrow().raft_groups();
        if self.behavior == MockBehavior::Empty {
            read.groups.clear();
        }
        apply_one_result(self.behavior, Ok(read)).await
    }

    async fn preview_leadership_transfer(
        &self,
        group: &str,
        target_id: &str,
    ) -> Result<TransferPreview, StudioError> {
        let read = self.cluster.borrow().preview_transfer(group, target_id);
        apply_one_result(self.behavior, read).await
    }

    async fn transfer_leadership(
        &self,
        group: &str,
        target_id: &str,
        revision: u64,
    ) -> Result<OpResult, StudioError> {
        apply_write(self.behavior, || {
            self.cluster
                .borrow_mut()
                .transfer(group, target_id, revision)
        })
        .await
    }

    async fn create_snapshot(&self, group: &str) -> Result<SnapshotInfo, StudioError> {
        apply_write(self.behavior, || {
            self.cluster.borrow_mut().create_snapshot(group)
        })
        .await
    }

    async fn placement(&self) -> Result<Placement, StudioError> {
        let mut read = self.cluster.borrow().placement();
        if self.behavior == MockBehavior::Empty {
            read.vshards.clear();
        }
        apply_one_result(self.behavior, Ok(read)).await
    }

    async fn preview_rebalance(&self, moves: &[MoveRequest]) -> Result<RebalancePlan, StudioError> {
        let read = self.cluster.borrow().preview_rebalance(moves);
        apply_one_result(self.behavior, read).await
    }

    async fn apply_rebalance(&self, plan: &RebalancePlan) -> Result<OpResult, StudioError> {
        apply_write(self.behavior, || {
            self.cluster.borrow_mut().apply_rebalance(plan)
        })
        .await
    }

    async fn telemetry(
        &self,
        node: Option<&str>,
        range: TelemetryRange,
    ) -> Result<Telemetry, StudioError> {
        let mut read = mock::telemetry(node, range);
        if self.behavior == MockBehavior::Empty {
            read.nodes.clear();
            read.series.clear();
        }
        apply_one_result(self.behavior, Ok(read)).await
    }

    async fn cluster_activity(&self, query: &ActivityQuery) -> Result<ActivityLog, StudioError> {
        let mut read = self.cluster.borrow().activity(query);
        if self.behavior == MockBehavior::Empty {
            read.events.clear();
        }
        apply_one_result(self.behavior, Ok(read)).await
    }
}
