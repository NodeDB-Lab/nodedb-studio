//! Admin-tier models. String fields mirror the wire, which returns every
//! scalar as a string. `is_superuser` and `enabled` are already plain `bool`
//! here: the real seam implementation will decode the server's "t"/"f"
//! encoding into these fields at that boundary, so views never have to see
//! the wire representation.

use serde::{Deserialize, Serialize};

/// One node in the cluster topology.
#[allow(dead_code)] // SEAM-UNWIRED
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterNode {
    pub id: String,
    pub address: String,
    pub state: String,
    pub raft_groups: String,
}

/// One Raft consensus group.
#[allow(dead_code)] // SEAM-UNWIRED
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaftGroup {
    pub id: String,
    pub role: String,
    pub leader_id: String,
    pub term: String,
    pub commit_index: String,
    pub last_applied: String,
    pub members: String,
}

/// One shard range and its current leaseholder.
#[allow(dead_code)] // SEAM-UNWIRED
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShardRange {
    pub id: String,
    pub group_id: String,
    pub leaseholder: String,
    pub replicas: String,
    pub qps: String,
    pub p99_ms: String,
}

/// One RBAC user row.
#[allow(dead_code)] // SEAM-UNWIRED
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserRow {
    pub id: String,
    pub username: String,
    pub tenant_id: String,
    pub roles: String,
    pub is_superuser: bool,
}

/// One row-level-security policy.
#[allow(dead_code)] // SEAM-UNWIRED
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RlsPolicy {
    pub id: String,
    pub name: String,
    pub collection: String,
    pub kind: String,
    pub mode: String,
    pub enabled: bool,
}

/// One audit-log entry.
#[allow(dead_code)] // SEAM-UNWIRED
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub when: String,
    pub actor: String,
    pub action: String,
    pub target: String,
    pub result: String,
}
