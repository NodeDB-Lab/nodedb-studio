//! Administration reads and changes. The state lives in `AdminStore`, so a
//! change shows in every later read and in the audit trail.

use async_trait::async_trait;

use crate::models::admin::{AdminChange, AdminReview};
use crate::models::admin_access::{AccessDirectory, AccessSubject, SubjectAccess};
use crate::models::admin_audit::{AuditEvent, AuditFacets, AuditPage, AuditQuery};
use crate::models::admin_backup::BackupOverview;
use crate::models::admin_config::{ConfigProposal, ConfigRequest, ConfigTarget, ConfigView};
use crate::models::admin_credential::{CredentialList, CredentialTest};
use crate::models::admin_policy::{PolicySet, PolicyTest, PolicyTestResult};
use crate::models::cluster_op::OpResult;
use crate::services::admin_data::AdminData;
use crate::services::error::StudioError;
use crate::services::mock::MockConnectionService;
use crate::services::mock_behavior::{MockBehavior, apply_one_result, apply_write};

#[async_trait(?Send)]
impl AdminData for MockConnectionService {
    async fn access_directory(&self) -> Result<AccessDirectory, StudioError> {
        let mut read = self.admin.borrow().directory();
        if self.behavior == MockBehavior::Empty {
            read.principals.clear();
        }
        apply_one_result(self.behavior, Ok(read)).await
    }

    async fn subject_access(&self, subject: &AccessSubject) -> Result<SubjectAccess, StudioError> {
        let read = self.admin.borrow().subject_access(subject);
        apply_one_result(self.behavior, read).await
    }

    async fn policy_set(&self, database: &str) -> Result<PolicySet, StudioError> {
        let read = self.admin.borrow().policy_set(database);
        let read = match (self.behavior, read) {
            (MockBehavior::Empty, Ok(mut set)) => {
                set.policies.clear();
                Ok(set)
            }
            (_, read) => read,
        };
        apply_one_result(self.behavior, read).await
    }

    async fn test_policy(&self, test: &PolicyTest) -> Result<PolicyTestResult, StudioError> {
        let read = self.admin.borrow().test_policy(test);
        apply_one_result(self.behavior, read).await
    }

    async fn audit_facets(&self) -> Result<AuditFacets, StudioError> {
        let read = self.admin.borrow().audit_facets();
        apply_one_result(self.behavior, Ok(read)).await
    }

    async fn audit_events(&self, query: &AuditQuery) -> Result<AuditPage, StudioError> {
        let mut read = self.admin.borrow().audit_page(query);
        if self.behavior == MockBehavior::Empty {
            read.events.clear();
            read.total = 0;
        }
        apply_one_result(self.behavior, Ok(read)).await
    }

    async fn audit_event(&self, id: &str) -> Result<AuditEvent, StudioError> {
        let read = self.admin.borrow().audit_event(id);
        apply_one_result(self.behavior, read).await
    }

    async fn backup_overview(&self) -> Result<BackupOverview, StudioError> {
        let mut read = self.admin.borrow().backup_overview();
        if self.behavior == MockBehavior::Empty {
            read.snapshots.clear();
            read.restore_jobs.clear();
        }
        apply_one_result(self.behavior, Ok(read)).await
    }

    async fn config_view(&self, target: &ConfigTarget) -> Result<ConfigView, StudioError> {
        let read = self.admin.borrow().config_view(target);
        let read = match (self.behavior, read) {
            (MockBehavior::Empty, Ok(mut v)) => {
                v.settings.clear();
                Ok(v)
            }
            (_, read) => read,
        };
        apply_one_result(self.behavior, read).await
    }

    async fn preview_config(&self, request: &ConfigRequest) -> Result<ConfigProposal, StudioError> {
        let read = self.admin.borrow().config_proposal(request);
        apply_one_result(self.behavior, read).await
    }

    async fn credentials(&self) -> Result<CredentialList, StudioError> {
        let mut read = self.admin.borrow().credential_list();
        if self.behavior == MockBehavior::Empty {
            read.items.clear();
        }
        apply_one_result(self.behavior, Ok(read)).await
    }

    async fn test_credential(&self, alias: &str) -> Result<CredentialTest, StudioError> {
        apply_write(self.behavior, || {
            self.admin.borrow_mut().test_credential(alias)
        })
        .await
    }

    async fn preview_admin_change(&self, change: &AdminChange) -> Result<AdminReview, StudioError> {
        let read = self.admin.borrow().preview(change);
        apply_one_result(self.behavior, read).await
    }

    async fn apply_admin_change(
        &self,
        change: &AdminChange,
        revision: u64,
    ) -> Result<OpResult, StudioError> {
        apply_write(self.behavior, || {
            self.admin.borrow_mut().apply(change, revision)
        })
        .await
    }
}
