//! Admin fixtures. Shapes mirror the server's introspection output so the
//! real implementation is a decoder swap, not a model change.

use crate::models::admin::{AuditEntry, ClusterNode, RaftGroup, RlsPolicy, ShardRange, UserRow};

/// Cluster topology: one row per node.
#[allow(dead_code)] // SEAM-UNWIRED
pub fn cluster_nodes() -> Vec<ClusterNode> {
    vec![
        ClusterNode {
            id: "1".into(),
            address: "127.0.0.1:6433".into(),
            state: "active".into(),
            raft_groups: "6".into(),
        },
        ClusterNode {
            id: "2".into(),
            address: "127.0.0.2:6433".into(),
            state: "active".into(),
            raft_groups: "5".into(),
        },
        ClusterNode {
            id: "3".into(),
            address: "127.0.0.3:6433".into(),
            state: "degraded".into(),
            raft_groups: "4".into(),
        },
    ]
}

/// Raft groups for the cluster.
#[allow(dead_code)] // SEAM-UNWIRED
pub fn raft_groups() -> Vec<RaftGroup> {
    (0..4)
        .map(|i| RaftGroup {
            id: i.to_string(),
            role: if i == 0 { "Leader" } else { "Follower" }.into(),
            leader_id: "1".into(),
            term: "1".into(),
            commit_index: (100 + i).to_string(),
            last_applied: (100 + i).to_string(),
            members: "1,2,3".into(),
        })
        .collect()
}

/// Shard ranges and their leaseholders.
#[allow(dead_code)] // SEAM-UNWIRED
pub fn shard_ranges() -> Vec<ShardRange> {
    (0..8)
        .map(|i| ShardRange {
            id: i.to_string(),
            group_id: ((i % 3) + 1).to_string(),
            leaseholder: ((i % 3) + 1).to_string(),
            replicas: "1,2,3".into(),
            qps: format!("{}.0", i * 12),
            p99_ms: format!("{}.5", i + 1),
        })
        .collect()
}

/// RBAC: all users in the tenant. `id` deliberately differs from `username`
/// (a `u-N` handle vs. the login name), same reasoning as
/// `streams::materialized_views`: a list keyed by the wrong field must be
/// visible instead of invisible.
#[allow(dead_code)] // SEAM-UNWIRED
pub fn users() -> Vec<UserRow> {
    vec![
        UserRow {
            id: "u-1".into(),
            username: "admin".into(),
            tenant_id: "1".into(),
            roles: "superuser".into(),
            is_superuser: true,
        },
        UserRow {
            id: "u-2".into(),
            username: "alice".into(),
            tenant_id: "1".into(),
            roles: "reader".into(),
            is_superuser: false,
        },
    ]
}

/// Row-level-security policies. `id` deliberately differs from `name`, same
/// reasoning as `users` above.
#[allow(dead_code)] // SEAM-UNWIRED
pub fn rls_policies() -> Vec<RlsPolicy> {
    vec![
        RlsPolicy {
            id: "rls-1".into(),
            name: "tenant_isolation".into(),
            collection: "orders".into(),
            kind: "select".into(),
            mode: "permissive".into(),
            enabled: true,
        },
        RlsPolicy {
            id: "rls-2".into(),
            name: "pii_masking".into(),
            collection: "users".into(),
            kind: "select".into(),
            mode: "restrictive".into(),
            enabled: false,
        },
    ]
}

/// Audit log entries.
#[allow(dead_code)] // SEAM-UNWIRED
pub fn audit_entries() -> Vec<AuditEntry> {
    (0..5)
        .map(|i| AuditEntry {
            id: format!("audit-{i}"),
            when: format!("2026-08-08 10:0{i}:00"),
            actor: "admin".into(),
            action: "SELECT".into(),
            target: "orders".into(),
            result: "allowed".into(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::mock::test_support::{assert_ids_distinct_from_names, assert_unique_ids};

    #[test]
    fn users_have_unique_ids_distinct_from_username() {
        let rows = users();
        assert_unique_ids(&rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>());
        assert_ids_distinct_from_names(rows.iter().map(|r| (r.id.as_str(), r.username.as_str())));
    }

    #[test]
    fn rls_policies_have_unique_ids_distinct_from_name() {
        let rows = rls_policies();
        assert_unique_ids(&rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>());
        assert_ids_distinct_from_names(rows.iter().map(|r| (r.id.as_str(), r.name.as_str())));
    }
}
