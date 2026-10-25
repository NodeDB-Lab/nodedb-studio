//! Admin fixtures. Shapes mirror the server's introspection output so the
//! real implementation is a decoder swap, not a model change.

use crate::models::admin::{AuditEntry, RlsPolicy, UserRow};

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
