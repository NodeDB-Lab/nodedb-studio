//! The administration state the mock serves. One store holds every tab's
//! objects, so a grant shows in the audit trail and a snapshot shows in the
//! notification it came from. Each domain keeps a revision. A preview reads
//! it, and apply names it: a stale review is refused.

use crate::data::mock::cluster::members::OBSERVED;
use crate::data::mock::connections::staging_databases;
use crate::models::admin::{AdminChange, AdminReview};
use crate::models::admin_access::{Principal, Role, RoleGrant};
use crate::models::admin_audit::{AuditEvent, AuditResult, RequestOrigin, TargetType};
use crate::models::admin_backup::{RestoreJob, Schedule, Snapshot};
use crate::models::admin_config::ConfigSetting;
use crate::models::admin_policy::Policy;
use crate::models::cluster_op::{OpResult, PermissionCheck};
use crate::services::error::StudioError;

pub(super) const ADMIN: &str = "admin";
pub(super) const DATABASE: &str = "analytics";

/// The revision of each domain. A write to a domain raises its number.
#[derive(Debug, Clone, Copy)]
pub(super) struct Revisions {
    pub access: u64,
    pub policies: u64,
    pub backups: u64,
    pub config: u64,
    pub credentials: u64,
}

#[derive(Debug, Clone)]
pub struct AdminStore {
    pub(super) clock: i64,
    pub(super) rev: Revisions,
    pub(super) principals: Vec<Principal>,
    pub(super) roles: Vec<Role>,
    pub(super) grants: Vec<RoleGrant>,
    /// `(principal id, role, database)`.
    pub(super) memberships: Vec<(String, String, String)>,
    pub(super) policies: Vec<Policy>,
    /// Newest first.
    pub(super) events: Vec<AuditEvent>,
    pub(super) snapshots: Vec<Snapshot>,
    pub(super) restores: Vec<RestoreJob>,
    pub(super) schedule: Schedule,
    pub(super) databases: Vec<String>,
    pub(super) cluster_config: Vec<ConfigSetting>,
    /// `(node id, key, value)`.
    pub(super) overrides: Vec<(String, String, String)>,
    pub(super) credentials: Vec<crate::models::admin_credential::Credential>,
    pub(super) next_audit: u32,
    pub(super) next_operation: u32,
    pub(super) next_principal: u32,
    pub(super) next_policy: u32,
    pub(super) next_restore: u32,
    granted: bool,
}

pub(super) fn missing(kind: &'static str, id: &str) -> StudioError {
    StudioError::UnknownObject {
        kind,
        id: id.to_string(),
    }
}

pub(super) fn stale(object: &str, expected: u64, stored: u64) -> StudioError {
    StudioError::StaleRevision {
        object: object.to_string(),
        expected,
        stored,
    }
}

impl AdminStore {
    pub fn seeded() -> Self {
        let (principals, roles, grants, memberships) = super::access::seeded_access();
        Self {
            clock: OBSERVED,
            rev: Revisions {
                access: 12,
                policies: 5,
                backups: 9,
                config: 7,
                credentials: 4,
            },
            principals,
            roles,
            grants,
            memberships,
            policies: super::policy::seeded_policies(),
            events: super::audit::seeded_events(),
            snapshots: super::backup::seeded_snapshots(),
            restores: super::backup::seeded_restores(),
            schedule: super::backup::seeded_schedule(),
            databases: staging_databases(),
            cluster_config: super::config::seeded_settings(),
            overrides: super::config::seeded_overrides(),
            credentials: super::credential::seeded_credentials(),
            next_audit: 105,
            next_operation: 91,
            next_principal: 4,
            next_policy: 3,
            next_restore: 104,
            granted: true,
        }
    }

    /// A store whose principal holds none of the privileges a change needs.
    #[cfg(test)]
    pub fn without_permission() -> Self {
        Self {
            granted: false,
            ..Self::seeded()
        }
    }

    pub(super) fn permission(&self, required: &str) -> PermissionCheck {
        PermissionCheck {
            required: required.to_string(),
            granted: self.granted,
        }
    }

    pub(super) fn operation(&mut self) -> String {
        let id = format!("op_{}", self.next_operation);
        self.next_operation += 1;
        id
    }

    /// Add one event to the trail at a new instant. Returns its id.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn record(
        &mut self,
        action: &str,
        target_type: TargetType,
        target: &str,
        object: &str,
        before: Vec<String>,
        after: Vec<String>,
    ) -> String {
        self.clock += 14;
        let id = format!("audit{}", self.next_audit);
        self.next_audit += 1;
        self.events.insert(
            0,
            AuditEvent {
                id: id.clone(),
                at: self.clock,
                actor: ADMIN.to_string(),
                action: action.to_string(),
                target_type,
                target: target.to_string(),
                database: DATABASE.to_string(),
                object: object.to_string(),
                result: AuditResult::Success,
                correlation: super::audit::correlation(self.next_audit),
                before,
                after,
                origin: RequestOrigin {
                    ip: "203.0.113.42".to_string(),
                    client: format!("NodeDB Studio {}", env!("CARGO_PKG_VERSION")),
                    method: "UI".to_string(),
                },
            },
        );
        id
    }

    pub fn preview(&self, change: &AdminChange) -> Result<AdminReview, StudioError> {
        match change {
            AdminChange::Access(c) => Ok(self.preview_access(c)),
            AdminChange::Policy(c) => Ok(self.preview_policy(c)),
            AdminChange::Backup(c) => Ok(self.preview_backup(c)),
            AdminChange::Config(c) => self.preview_config_review(c),
            AdminChange::Credential(c) => self.preview_credential(c),
        }
    }

    /// Apply a change against the revision its preview read.
    pub fn apply(&mut self, change: &AdminChange, revision: u64) -> Result<OpResult, StudioError> {
        let review = self.preview(change)?;
        if review.revision != revision {
            return Err(stale(&review.target, revision, review.revision));
        }
        if !review.permission.granted {
            return Err(StudioError::Rejected {
                reason: format!("the {} privilege is required", review.permission.required),
            });
        }
        if let Some(reason) = review.errors.first() {
            return Err(StudioError::Rejected {
                reason: reason.clone(),
            });
        }
        let summary = match change {
            AdminChange::Access(c) => self.apply_access(c),
            AdminChange::Policy(c) => self.apply_policy(c),
            AdminChange::Backup(c) => self.apply_backup(c),
            AdminChange::Config(c) => self.apply_config(c),
            AdminChange::Credential(c) => self.apply_credential(c),
        }?;
        let operation = self.operation();
        Ok(OpResult { operation, summary })
    }
}
