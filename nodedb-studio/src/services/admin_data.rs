//! Administration reads and changes at the backend seam: access, row
//! policies, the audit trail, backups, config, and credentials.
//!
//! Every change is previewed first. The preview carries the revision it
//! read, and apply names that revision, so a stale review is refused with
//! `StaleRevision` and nothing is overwritten. An applied change adds an
//! event to the audit trail.

use async_trait::async_trait;

use crate::models::admin::{AdminChange, AdminReview};
use crate::models::admin_access::{AccessDirectory, AccessSubject, SubjectAccess};
use crate::models::admin_audit::{AuditEvent, AuditFacets, AuditPage, AuditQuery};
use crate::models::admin_backup::BackupOverview;
use crate::models::admin_config::{ConfigProposal, ConfigRequest, ConfigTarget, ConfigView};
use crate::models::admin_credential::{CredentialList, CredentialTest};
use crate::models::admin_policy::{PolicySet, PolicyTest, PolicyTestResult};
use crate::models::cluster_op::OpResult;
use crate::services::error::StudioError;

#[async_trait(?Send)]
pub trait AdminData {
    /// Every principal and role, with the databases roles can be held in.
    async fn access_directory(&self) -> Result<AccessDirectory, StudioError>;

    /// The memberships, grants, and effective access of one principal or role.
    async fn subject_access(&self, subject: &AccessSubject) -> Result<SubjectAccess, StudioError>;

    /// The row policies of one database, with what the editor offers.
    async fn policy_set(&self, database: &str) -> Result<PolicySet, StudioError>;

    /// Run a policy filter as a user and read the rows it keeps. Writes nothing.
    async fn test_policy(&self, test: &PolicyTest) -> Result<PolicyTestResult, StudioError>;

    /// The actors and actions the filters list, and the retention statement.
    async fn audit_facets(&self) -> Result<AuditFacets, StudioError>;

    /// One page of events matching `query`, newest first.
    async fn audit_events(&self, query: &AuditQuery) -> Result<AuditPage, StudioError>;

    /// One event, whatever page it is on.
    async fn audit_event(&self, id: &str) -> Result<AuditEvent, StudioError>;

    /// Snapshots, restore jobs, the schedule, and the restore range.
    async fn backup_overview(&self) -> Result<BackupOverview, StudioError>;

    /// The settings of the cluster or one node. An unreachable node is an error.
    async fn config_view(&self, target: &ConfigTarget) -> Result<ConfigView, StudioError>;

    /// What `request` would change. Nothing changes.
    async fn preview_config(&self, request: &ConfigRequest) -> Result<ConfigProposal, StudioError>;

    /// The credential aliases and the vector indexes their dimensions must fit.
    async fn credentials(&self) -> Result<CredentialList, StudioError>;

    /// Send one explicit request to the provider. It never runs on its own.
    async fn test_credential(&self, alias: &str) -> Result<CredentialTest, StudioError>;

    /// What `change` would do, and the revision the preview read.
    async fn preview_admin_change(&self, change: &AdminChange) -> Result<AdminReview, StudioError>;

    /// Apply a previewed change. `revision` is the preview's.
    async fn apply_admin_change(
        &self,
        change: &AdminChange,
        revision: u64,
    ) -> Result<OpResult, StudioError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::admin_access::AccessChange;
    use crate::models::admin_audit::AuditRange;
    use crate::models::admin_backup::{BackupChange, RestoreRequest};
    use crate::models::admin_config::ConfigScope;
    use crate::models::admin_policy::{PolicyChange, PolicyOperation};
    use crate::models::format::epoch;
    use crate::services::async_state::AsyncState;
    use crate::services::mock::MockConnectionService;

    fn unique<'a>(it: impl Iterator<Item = &'a str>, what: &str) {
        let mut v: Vec<&str> = it.collect();
        let total = v.len();
        assert!(total > 0, "{what} fixture must not be empty");
        v.sort_unstable();
        v.dedup();
        assert_eq!(total, v.len(), "{what} ids must be unique");
    }

    fn week() -> AuditQuery {
        AuditQuery {
            range: AuditRange::Last7Days,
            page_size: 10,
            ..AuditQuery::default()
        }
    }

    fn node2() -> ConfigTarget {
        ConfigTarget {
            scope: ConfigScope::Node,
            node: Some("nd_02".into()),
        }
    }

    #[tokio::test]
    async fn every_admin_list_has_unique_ids() {
        let svc = MockConnectionService::ready();
        let dir = svc.access_directory().await.expect("directory");
        unique(dir.principals.iter().map(|p| p.id.as_str()), "principals");
        unique(
            dir.principals.iter().map(|p| p.name.as_str()),
            "principal names",
        );
        unique(dir.roles.iter().map(|r| r.name.as_str()), "roles");
        let policies = svc.policy_set("analytics").await.expect("policies");
        unique(policies.policies.iter().map(|p| p.id.as_str()), "policies");
        let page = svc.audit_events(&week()).await.expect("audit");
        unique(page.events.iter().map(|e| e.id.as_str()), "audit events");
        let backups = svc.backup_overview().await.expect("backups");
        unique(backups.snapshots.iter().map(|s| s.id.as_str()), "snapshots");
        unique(
            backups.restore_jobs.iter().map(|r| r.id.as_str()),
            "restores",
        );
        let creds = svc.credentials().await.expect("credentials");
        unique(creds.items.iter().map(|c| c.alias.as_str()), "credentials");
    }

    #[tokio::test]
    async fn every_list_is_empty_when_the_backend_is() {
        let svc = MockConnectionService::empty();
        assert!(AsyncState::from_value(Some(svc.access_directory().await)).is_empty());
        assert!(AsyncState::from_value(Some(svc.policy_set("analytics").await)).is_empty());
        assert!(AsyncState::from_value(Some(svc.audit_events(&week()).await)).is_empty());
        assert!(AsyncState::from_value(Some(svc.backup_overview().await)).is_empty());
        assert!(AsyncState::from_value(Some(svc.config_view(&node2()).await)).is_empty());
        assert!(AsyncState::from_value(Some(svc.credentials().await)).is_empty());
    }

    #[tokio::test]
    async fn every_read_errors_when_the_backend_does() {
        let svc = MockConnectionService::erroring();
        assert!(
            AsyncState::from_value(Some(svc.access_directory().await))
                .error_message()
                .is_some()
        );
        assert!(
            AsyncState::from_value(Some(svc.policy_set("analytics").await))
                .error_message()
                .is_some()
        );
        assert!(
            AsyncState::from_value(Some(svc.audit_events(&week()).await))
                .error_message()
                .is_some()
        );
        assert!(
            AsyncState::from_value(Some(svc.audit_facets().await))
                .error_message()
                .is_some()
        );
        assert!(
            AsyncState::from_value(Some(svc.backup_overview().await))
                .error_message()
                .is_some()
        );
        assert!(
            AsyncState::from_value(Some(svc.config_view(&node2()).await))
                .error_message()
                .is_some()
        );
        assert!(
            AsyncState::from_value(Some(svc.credentials().await))
                .error_message()
                .is_some()
        );
        assert!(
            svc.preview_admin_change(&AdminChange::Access(vec![]))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn a_grant_previews_applies_and_shows_in_the_audit_trail() {
        let svc = MockConnectionService::ready();
        let change = AdminChange::Access(vec![AccessChange::AddMembership {
            principal: "pr_03".into(),
            role: "analytics".into(),
            database: "analytics".into(),
        }]);
        let review = svc.preview_admin_change(&change).await.expect("preview");
        assert!(review.can_apply(), "{:?}", review.errors);
        svc.apply_admin_change(&change, review.revision)
            .await
            .expect("apply");
        let access = svc
            .subject_access(&AccessSubject::Principal("pr_03".into()))
            .await
            .expect("access");
        assert_eq!(access.memberships.len(), 2);
        let page = svc.audit_events(&week()).await.expect("audit");
        assert_eq!(page.total, 48);
        let e = &page.events[0];
        assert_eq!(
            (e.id.as_str(), e.action.as_str()),
            ("audit105", "Grant role")
        );
        assert_eq!(e.target, "service:ingest_bot");
    }

    #[tokio::test]
    async fn a_stale_review_never_applies() {
        let svc = MockConnectionService::ready();
        let change = AdminChange::Access(vec![AccessChange::RemoveMembership {
            principal: "pr_03".into(),
            role: "ingest_writer".into(),
            database: "analytics".into(),
        }]);
        let first = svc.preview_admin_change(&change).await.expect("preview");
        let other = AdminChange::Access(vec![AccessChange::CreateRole {
            name: "auditor".into(),
            description: String::new(),
        }]);
        let r = svc.preview_admin_change(&other).await.expect("preview");
        svc.apply_admin_change(&other, r.revision)
            .await
            .expect("apply");
        let err = svc
            .apply_admin_change(&change, first.revision)
            .await
            .expect_err("stale");
        assert!(matches!(err, StudioError::StaleRevision { .. }), "{err}");
        let access = svc
            .subject_access(&AccessSubject::Principal("pr_03".into()))
            .await
            .expect("access");
        assert_eq!(access.memberships.len(), 1, "the removal did not apply");
    }

    #[tokio::test]
    async fn a_principal_without_the_privilege_cannot_apply() {
        let svc = MockConnectionService::ready().without_admin_permission();
        let change = AdminChange::Access(vec![AccessChange::CreateRole {
            name: "auditor".into(),
            description: String::new(),
        }]);
        let review = svc.preview_admin_change(&change).await.expect("preview");
        assert!(!review.permission.granted && !review.can_apply());
        assert!(
            svc.apply_admin_change(&change, review.revision)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn an_erroring_backend_refuses_a_write_before_it_changes_anything() {
        let ready = MockConnectionService::ready();
        let svc = ready.with_shared_state(crate::services::mock_behavior::MockBehavior::Erroring);
        let change = AdminChange::Access(vec![AccessChange::CreateRole {
            name: "auditor".into(),
            description: String::new(),
        }]);
        assert!(svc.apply_admin_change(&change, 12).await.is_err());
        let dir = ready.access_directory().await.expect("directory");
        assert!(!dir.roles.iter().any(|r| r.name == "auditor"));
    }

    #[tokio::test]
    async fn a_policy_edit_applies_and_records_its_audit_event() {
        let svc = MockConnectionService::ready();
        let set = svc.policy_set("analytics").await.expect("policies");
        let mut draft = set.policies[0].draft();
        draft.enabled = false;
        let change = AdminChange::Policy(PolicyChange::Update(draft));
        let review = svc.preview_admin_change(&change).await.expect("preview");
        assert!(review.can_apply(), "{:?}", review.errors);
        assert!(!review.warnings.is_empty(), "disabling warns");
        svc.apply_admin_change(&change, review.revision)
            .await
            .expect("apply");
        let after = svc.policy_set("analytics").await.expect("policies");
        assert!(!after.policies[0].enabled);
        let page = svc.audit_events(&week()).await.expect("audit");
        assert_eq!(page.events[0].action, "Update policy");
    }

    #[tokio::test]
    async fn a_policy_that_writes_needs_a_check() {
        let svc = MockConnectionService::ready();
        let set = svc.policy_set("analytics").await.expect("policies");
        let mut draft = set.policies[0].draft();
        draft.operation = PolicyOperation::Insert;
        let review = svc
            .preview_admin_change(&AdminChange::Policy(PolicyChange::Update(draft)))
            .await
            .expect("preview");
        assert!(!review.can_apply());
    }

    #[tokio::test]
    async fn a_restore_creates_a_job_and_takes_the_database_name() {
        let svc = MockConnectionService::ready();
        let o = svc.backup_overview().await.expect("backups");
        let change = AdminChange::Backup(BackupChange::Restore(RestoreRequest {
            snapshot: o.snapshots[0].id.clone(),
            target: "analytics_restore".into(),
            point: epoch(2026, 6, 13, 2, 0, 0),
        }));
        let review = svc.preview_admin_change(&change).await.expect("preview");
        assert!(review.can_apply(), "{:?}", review.errors);
        svc.apply_admin_change(&change, review.revision)
            .await
            .expect("apply");
        let after = svc.backup_overview().await.expect("backups");
        assert_eq!(after.restore_jobs[0].id, "restore104");
        let again = svc.preview_admin_change(&change).await.expect("preview");
        assert!(!again.can_apply(), "the name is taken now");
    }

    #[tokio::test]
    async fn a_config_change_applies_to_one_node_only() {
        let svc = MockConnectionService::ready();
        let req = ConfigRequest {
            target: node2(),
            changes: vec![("query_timeout".into(), "60s".into())],
        };
        let p = svc.preview_config(&req).await.expect("proposal");
        assert!(p.can_apply());
        svc.apply_admin_change(&AdminChange::Config(req), p.revision)
            .await
            .expect("apply");
        let n2 = svc.config_view(&node2()).await.expect("node 2");
        let n3 = svc
            .config_view(&ConfigTarget {
                scope: ConfigScope::Node,
                node: Some("nd_03".into()),
            })
            .await
            .expect("node 3");
        let value = |v: &ConfigView| {
            v.settings
                .iter()
                .find(|s| s.key == "query_timeout")
                .map(|s| s.current.clone())
        };
        assert_eq!(value(&n2).as_deref(), Some("60s"));
        assert_eq!(value(&n3).as_deref(), Some("30s"));
    }

    #[tokio::test]
    async fn a_new_credential_previews_applies_and_reads_unknown() {
        use crate::models::admin_credential::{
            CredentialChange, CredentialDraft, CredentialStatus,
        };
        let svc = MockConnectionService::ready();
        let mut d = CredentialDraft::blank();
        d.alias = "ocr_provider".into();
        d.secret_ref = "secret/ocr".into();
        d.endpoint = "https://ocr.internal".into();
        d.models = "ocr-v1".into();
        let change = AdminChange::Credential(CredentialChange::Create(d.clone()));
        let review = svc.preview_admin_change(&change).await.expect("preview");
        assert!(review.can_apply(), "{:?}", review.errors);
        svc.apply_admin_change(&change, review.revision)
            .await
            .expect("apply");
        let list = svc.credentials().await.expect("credentials");
        let made = list
            .items
            .iter()
            .find(|c| c.alias == "ocr_provider")
            .expect("created");
        assert_eq!(made.status, CredentialStatus::Unknown);
        let again = svc.preview_admin_change(&change).await.expect("preview");
        assert!(!again.can_apply(), "the alias is taken now");
        let page = svc.audit_events(&week()).await.expect("audit");
        assert_eq!(page.events[0].action, "Create credential");
    }

    #[tokio::test]
    async fn a_credential_test_is_the_only_call_to_the_provider() {
        let svc = MockConnectionService::ready();
        let before = svc.credentials().await.expect("credentials");
        assert_eq!(
            before.items[1].status,
            crate::models::admin_credential::CredentialStatus::Unknown
        );
        let t = svc.test_credential("backup_store").await.expect("test");
        assert!(t.reachable);
    }
}
