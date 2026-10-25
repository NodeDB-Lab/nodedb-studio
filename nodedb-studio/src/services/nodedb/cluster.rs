//! Cluster reads and operations.

use async_trait::async_trait;

use crate::models::cluster::{ClusterMembers, HostedGroup};
use crate::models::cluster_activity::{ActivityLog, ActivityQuery};
use crate::models::cluster_monitor::{Telemetry, TelemetryRange};
use crate::models::cluster_op::OpResult;
use crate::models::cluster_place::{MoveRequest, Placement, RebalancePlan};
use crate::models::cluster_raft::{RaftGroups, SnapshotInfo, TransferPreview};
use crate::services::cluster_data::ClusterData;
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;

#[async_trait(?Send)]
impl ClusterData for NodeDbConnectionService {
    async fn cluster_members(&self) -> Result<ClusterMembers, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn hosted_groups(&self, _node_id: &str) -> Result<Vec<HostedGroup>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn raft_groups(&self) -> Result<RaftGroups, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn preview_leadership_transfer(
        &self,
        _group: &str,
        _target_id: &str,
    ) -> Result<TransferPreview, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn transfer_leadership(
        &self,
        _group: &str,
        _target_id: &str,
        _revision: u64,
    ) -> Result<OpResult, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn create_snapshot(&self, _group: &str) -> Result<SnapshotInfo, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn placement(&self) -> Result<Placement, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn preview_rebalance(
        &self,
        _moves: &[MoveRequest],
    ) -> Result<RebalancePlan, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn apply_rebalance(&self, _plan: &RebalancePlan) -> Result<OpResult, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn telemetry(
        &self,
        _node: Option<&str>,
        _range: TelemetryRange,
    ) -> Result<Telemetry, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn cluster_activity(&self, _query: &ActivityQuery) -> Result<ActivityLog, StudioError> {
        Err(StudioError::NotConnected)
    }
}
