//! Administration reads and changes.

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
use crate::services::nodedb::NodeDbConnectionService;

#[async_trait(?Send)]
impl AdminData for NodeDbConnectionService {
    async fn access_directory(&self) -> Result<AccessDirectory, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn subject_access(&self, _subject: &AccessSubject) -> Result<SubjectAccess, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn policy_set(&self, _database: &str) -> Result<PolicySet, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn test_policy(&self, _test: &PolicyTest) -> Result<PolicyTestResult, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn audit_facets(&self) -> Result<AuditFacets, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn audit_events(&self, _query: &AuditQuery) -> Result<AuditPage, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn audit_event(&self, _id: &str) -> Result<AuditEvent, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn backup_overview(&self) -> Result<BackupOverview, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn config_view(&self, _target: &ConfigTarget) -> Result<ConfigView, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn preview_config(
        &self,
        _request: &ConfigRequest,
    ) -> Result<ConfigProposal, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn credentials(&self) -> Result<CredentialList, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn test_credential(&self, _alias: &str) -> Result<CredentialTest, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn preview_admin_change(
        &self,
        _change: &AdminChange,
    ) -> Result<AdminReview, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn apply_admin_change(
        &self,
        _change: &AdminChange,
        _revision: u64,
    ) -> Result<OpResult, StudioError> {
        Err(StudioError::NotConnected)
    }
}
