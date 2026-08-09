//! Admin-tier reads at the backend seam: cluster, raft, shards, RBAC, RLS,
//! audit. Every one has a real server-side source, so these signatures are
//! designed for a real implementation rather than as permanent mock stubs.

use async_trait::async_trait;

use crate::models::admin::{AuditEntry, ClusterNode, RaftGroup, RlsPolicy, ShardRange, UserRow};
use crate::services::error::StudioError;

#[async_trait(?Send)]
pub trait AdminData {
    /// Cluster topology: one row per node.
    #[allow(dead_code)] // SEAM-UNWIRED
    async fn cluster_nodes(&self) -> Result<Vec<ClusterNode>, StudioError>;

    /// Raft groups for the cluster.
    #[allow(dead_code)] // SEAM-UNWIRED
    async fn raft_groups(&self) -> Result<Vec<RaftGroup>, StudioError>;

    /// Shard ranges and their leaseholders.
    #[allow(dead_code)] // SEAM-UNWIRED
    async fn shard_ranges(&self) -> Result<Vec<ShardRange>, StudioError>;

    /// RBAC: all users in the tenant.
    #[allow(dead_code)] // SEAM-UNWIRED
    async fn users(&self) -> Result<Vec<UserRow>, StudioError>;

    /// Row-level-security policies.
    #[allow(dead_code)] // SEAM-UNWIRED
    async fn rls_policies(&self) -> Result<Vec<RlsPolicy>, StudioError>;

    /// Audit log entries.
    #[allow(dead_code)] // SEAM-UNWIRED
    async fn audit_entries(&self) -> Result<Vec<AuditEntry>, StudioError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::async_state::AsyncState;
    use crate::services::connection_service::MockConnectionService;

    #[tokio::test]
    async fn every_admin_read_has_unique_ids() {
        let svc = MockConnectionService::ready();
        let nodes = svc.cluster_nodes().await.expect("nodes");
        let groups = svc.raft_groups().await.expect("raft");
        let shards = svc.shard_ranges().await.expect("shards");
        let users = svc.users().await.expect("users");
        let policies = svc.rls_policies().await.expect("rls");
        let audit = svc.audit_entries().await.expect("audit");

        assert_unique(nodes.iter().map(|x| x.id.as_str()), "cluster_nodes");
        assert_unique(groups.iter().map(|x| x.id.as_str()), "raft_groups");
        assert_unique(shards.iter().map(|x| x.id.as_str()), "shard_ranges");
        assert_unique(users.iter().map(|x| x.id.as_str()), "users");
        assert_unique(policies.iter().map(|x| x.id.as_str()), "rls_policies");
        assert_unique(audit.iter().map(|x| x.id.as_str()), "audit_entries");
    }

    fn assert_unique<'a>(it: impl Iterator<Item = &'a str>, what: &str) {
        let mut v: Vec<&str> = it.collect();
        let total = v.len();
        assert!(total > 0, "{what} fixture must not be empty");
        v.sort_unstable();
        v.dedup();
        assert_eq!(total, v.len(), "{what} ids must be unique");
    }

    // Each method gets its own empty/erroring pair rather than one combined
    // check per behaviour: `apply(self.behavior, mock::x)` and a mis-wired
    // `Ok(mock::x())` both satisfy a single shared assertion, so every method
    // needs its own proof that it actually reads `self.behavior`.

    #[tokio::test]
    async fn cluster_nodes_empty_is_empty() {
        let svc = MockConnectionService::empty();
        let s = AsyncState::from_value(Some(svc.cluster_nodes().await));
        assert!(s.is_empty());
    }

    #[tokio::test]
    async fn cluster_nodes_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.cluster_nodes().await));
        assert!(s.error_message().is_some());
    }

    #[tokio::test]
    async fn raft_groups_empty_is_empty() {
        let svc = MockConnectionService::empty();
        let s = AsyncState::from_value(Some(svc.raft_groups().await));
        assert!(s.is_empty());
    }

    #[tokio::test]
    async fn raft_groups_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.raft_groups().await));
        assert!(s.error_message().is_some());
    }

    #[tokio::test]
    async fn shard_ranges_empty_is_empty() {
        let svc = MockConnectionService::empty();
        let s = AsyncState::from_value(Some(svc.shard_ranges().await));
        assert!(s.is_empty());
    }

    #[tokio::test]
    async fn shard_ranges_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.shard_ranges().await));
        assert!(s.error_message().is_some());
    }

    #[tokio::test]
    async fn users_empty_is_empty() {
        let svc = MockConnectionService::empty();
        let s = AsyncState::from_value(Some(svc.users().await));
        assert!(s.is_empty());
    }

    #[tokio::test]
    async fn users_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.users().await));
        assert!(s.error_message().is_some());
    }

    #[tokio::test]
    async fn rls_policies_empty_is_empty() {
        let svc = MockConnectionService::empty();
        let s = AsyncState::from_value(Some(svc.rls_policies().await));
        assert!(s.is_empty());
    }

    #[tokio::test]
    async fn rls_policies_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.rls_policies().await));
        assert!(s.error_message().is_some());
    }

    #[tokio::test]
    async fn audit_entries_empty_is_empty() {
        let svc = MockConnectionService::empty();
        let s = AsyncState::from_value(Some(svc.audit_entries().await));
        assert!(s.is_empty());
    }

    #[tokio::test]
    async fn audit_entries_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.audit_entries().await));
        assert!(s.error_message().is_some());
    }

    #[tokio::test]
    async fn users_fixture_marks_admin_superuser_and_alice_not() {
        let svc = MockConnectionService::ready();
        let users = svc.users().await.expect("users");
        let admin = users
            .iter()
            .find(|u| u.username == "admin")
            .expect("fixture has an `admin` user");
        let alice = users
            .iter()
            .find(|u| u.username == "alice")
            .expect("fixture has an `alice` user");
        assert!(admin.is_superuser, "admin fixture must be a superuser");
        assert!(!alice.is_superuser, "alice fixture must not be a superuser");
    }

    #[tokio::test]
    async fn rls_policies_fixture_enabled_flags_match_intent() {
        let svc = MockConnectionService::ready();
        let policies = svc.rls_policies().await.expect("rls");
        let tenant_isolation = policies
            .iter()
            .find(|p| p.name == "tenant_isolation")
            .expect("fixture has a `tenant_isolation` policy");
        let pii_masking = policies
            .iter()
            .find(|p| p.name == "pii_masking")
            .expect("fixture has a `pii_masking` policy");
        assert!(
            tenant_isolation.enabled,
            "tenant_isolation must be enabled in the fixture"
        );
        assert!(
            !pii_masking.enabled,
            "pii_masking must be disabled in the fixture"
        );
    }
}
