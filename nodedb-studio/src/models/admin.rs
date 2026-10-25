//! Admin-tier models. String fields mirror the wire, which returns every
//! scalar as a string. `is_superuser` and `enabled` are already plain `bool`
//! here: the real seam implementation will decode the server's "t"/"f"
//! encoding into these fields at that boundary, so views never have to see
//! the wire representation.

use serde::{Deserialize, Serialize};

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
