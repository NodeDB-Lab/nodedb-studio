//! Cluster reads and operations. Scope is the whole Origin cluster: none of
//! these depend on the selected database. A membership operation is read as a
//! preview first. Apply names the revision the preview read and the server
//! refuses it when the cluster changed since.

use async_trait::async_trait;

use crate::models::cluster::{ClusterMembers, HostedGroup};
use crate::models::cluster_activity::{ActivityLog, ActivityQuery};
use crate::models::cluster_monitor::{Telemetry, TelemetryRange};
use crate::models::cluster_op::OpResult;
use crate::models::cluster_place::{MoveRequest, Placement, RebalancePlan};
use crate::models::cluster_raft::{RaftGroups, SnapshotInfo, TransferPreview};
use crate::services::error::StudioError;

#[async_trait(?Send)]
pub trait ClusterData {
    /// Every configured member with its observed reachability.
    async fn cluster_members(&self) -> Result<ClusterMembers, StudioError>;

    /// The Raft groups member `node_id` hosts, as that member observes them.
    async fn hosted_groups(&self, node_id: &str) -> Result<Vec<HostedGroup>, StudioError>;

    /// Every Raft group with its peers, log positions, and snapshots.
    async fn raft_groups(&self) -> Result<RaftGroups, StudioError>;

    /// What moving `group`'s leadership to member `target_id` will do.
    /// Refused when the target is not a reachable voter of the group.
    async fn preview_leadership_transfer(
        &self,
        group: &str,
        target_id: &str,
    ) -> Result<TransferPreview, StudioError>;

    /// Move `group`'s leadership. `revision` is the preview's.
    async fn transfer_leadership(
        &self,
        group: &str,
        target_id: &str,
        revision: u64,
    ) -> Result<OpResult, StudioError>;

    /// Snapshot `group`'s state machine at its leader's applied index.
    async fn create_snapshot(&self, group: &str) -> Result<SnapshotInfo, StudioError>;

    /// Every vShard with its group, replicas, and the node disks.
    async fn placement(&self) -> Result<Placement, StudioError>;

    /// What the staged replica moves will do. Nothing changes.
    async fn preview_rebalance(&self, moves: &[MoveRequest]) -> Result<RebalancePlan, StudioError>;

    /// Start the previewed plan. It runs as one operation with a stable id.
    async fn apply_rebalance(&self, plan: &RebalancePlan) -> Result<OpResult, StudioError>;

    /// Telemetry over `range`. `node` is a member id, or every node together
    /// when `None`.
    async fn telemetry(
        &self,
        node: Option<&str>,
        range: TelemetryRange,
    ) -> Result<Telemetry, StudioError>;

    /// Membership changes and administrative operations matching `query`,
    /// newest first.
    async fn cluster_activity(&self, query: &ActivityQuery) -> Result<ActivityLog, StudioError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::async_state::AsyncState;
    use crate::services::mock::MockConnectionService;

    #[tokio::test]
    async fn every_member_hosts_at_least_one_group() {
        let svc = MockConnectionService::ready();
        let members = svc.cluster_members().await.expect("members");
        for m in &members.members {
            let groups = svc.hosted_groups(&m.id).await.expect("groups");
            assert!(!groups.is_empty(), "{} hosts no group", m.id);
        }
    }

    #[tokio::test]
    async fn member_ids_are_unique() {
        let svc = MockConnectionService::ready();
        let members = svc.cluster_members().await.expect("members");
        let mut ids: Vec<_> = members.members.iter().map(|m| &m.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), members.members.len());
    }

    #[tokio::test]
    async fn group_and_vshard_ids_are_unique() {
        let svc = MockConnectionService::ready();
        let groups = svc.raft_groups().await.expect("groups");
        let mut ids: Vec<_> = groups.groups.iter().map(|g| g.id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), groups.groups.len());
        let placement = svc.placement().await.expect("placement");
        let mut nums: Vec<_> = placement.vshards.iter().map(|v| v.number).collect();
        nums.sort();
        nums.dedup();
        assert_eq!(nums.len(), placement.vshards.len());
        // Every vShard names a group that exists, and its replicas are the group's.
        for v in &placement.vshards {
            let g = groups.group(&v.group).expect("group of vshard");
            assert_eq!(v.replicas.len(), g.peers.len(), "vShard {}", v.number);
            assert_eq!(v.leader, g.leader_name());
        }
    }

    #[tokio::test]
    async fn a_transfer_through_the_seam_shows_in_every_read() {
        let svc = MockConnectionService::ready();
        let groups = svc.raft_groups().await.expect("groups");
        let preview = svc
            .preview_leadership_transfer("data-11", "nd_02")
            .await
            .expect("preview");
        assert!(preview.can_apply());
        let out = svc
            .transfer_leadership("data-11", "nd_02", preview.revision)
            .await
            .expect("transfer");
        assert!(out.operation.starts_with("op_"));
        let after = svc.raft_groups().await.expect("groups");
        assert!(after.revision > groups.revision);
        assert_eq!(after.group("data-11").expect("g").leader_name(), "node-2");
        let hosted = svc.hosted_groups("nd_02").await.expect("hosted");
        assert!(
            hosted
                .iter()
                .any(|g| g.group == "data-11" && g.leader == "node-2")
        );
        let log = svc
            .cluster_activity(&ActivityQuery::default())
            .await
            .expect("log");
        assert_eq!(
            log.events[0].correlation.as_deref(),
            Some(out.operation.as_str())
        );
        // The preview is spent: applying it again is refused.
        assert!(
            svc.transfer_leadership("data-11", "nd_02", preview.revision)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn telemetry_follows_the_node_filter_and_range() {
        let svc = MockConnectionService::ready();
        let one = svc
            .telemetry(Some("nd_01"), TelemetryRange::Hour1)
            .await
            .expect("one");
        assert_eq!(one.scope.as_deref(), Some("nd_01"));
        let short = svc
            .telemetry(None, TelemetryRange::Minutes15)
            .await
            .expect("all");
        assert_eq!(short.scope, None);
        assert_eq!(short.range, TelemetryRange::Minutes15);
        assert_eq!(short.nodes.len(), 5);
    }

    #[tokio::test]
    async fn empty_behaviour_reaches_the_empty_state() {
        let svc = MockConnectionService::empty();
        let state = AsyncState::from_value(Some(svc.cluster_members().await));
        assert!(state.is_empty());
        let state = AsyncState::from_value(Some(svc.raft_groups().await));
        assert!(state.is_empty());
        let state = AsyncState::from_value(Some(svc.placement().await));
        assert!(state.is_empty());
        let state = AsyncState::from_value(Some(svc.telemetry(None, TelemetryRange::Hour1).await));
        assert!(state.is_empty());
        let state =
            AsyncState::from_value(Some(svc.cluster_activity(&ActivityQuery::default()).await));
        assert!(state.is_empty());
    }

    #[tokio::test]
    async fn erroring_behaviour_reaches_the_error_state() {
        let svc = MockConnectionService::erroring();
        assert!(svc.cluster_members().await.is_err());
        assert!(svc.hosted_groups("nd_01").await.is_err());
        assert!(svc.raft_groups().await.is_err());
        assert!(svc.placement().await.is_err());
        assert!(svc.telemetry(None, TelemetryRange::Hour1).await.is_err());
        assert!(
            svc.cluster_activity(&ActivityQuery::default())
                .await
                .is_err()
        );
        assert!(svc.create_snapshot("data-11").await.is_err());
        assert!(
            svc.preview_leadership_transfer("data-11", "nd_02")
                .await
                .is_err()
        );
    }
}
