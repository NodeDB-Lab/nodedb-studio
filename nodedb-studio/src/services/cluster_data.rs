//! Cluster membership and per-member replication reads. Scope is the whole
//! Origin cluster: none of these depend on the selected database.

use async_trait::async_trait;

use crate::models::cluster::{ClusterMembers, HostedGroup};
use crate::services::error::StudioError;

#[async_trait(?Send)]
pub trait ClusterData {
    /// Every configured member with its observed reachability.
    async fn cluster_members(&self) -> Result<ClusterMembers, StudioError>;

    /// The Raft groups member `node_id` hosts, as that member observes them.
    async fn hosted_groups(&self, node_id: &str) -> Result<Vec<HostedGroup>, StudioError>;
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
    async fn empty_behaviour_reaches_the_empty_state() {
        let svc = MockConnectionService::empty();
        let state = AsyncState::from_value(Some(svc.cluster_members().await));
        assert!(state.is_empty());
    }

    #[tokio::test]
    async fn erroring_behaviour_reaches_the_error_state() {
        let svc = MockConnectionService::erroring();
        assert!(svc.cluster_members().await.is_err());
        assert!(svc.hosted_groups("nd_01").await.is_err());
    }
}
