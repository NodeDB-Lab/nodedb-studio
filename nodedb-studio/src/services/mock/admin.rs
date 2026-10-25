//! Administration reads.

use async_trait::async_trait;

use crate::data::mock;
use crate::models::admin::{AuditEntry, RlsPolicy, UserRow};
use crate::services::admin_data::AdminData;
use crate::services::error::StudioError;
use crate::services::mock::MockConnectionService;
use crate::services::mock_behavior::apply;

#[async_trait(?Send)]
impl AdminData for MockConnectionService {
    async fn users(&self) -> Result<Vec<UserRow>, StudioError> {
        apply(self.behavior, mock::users).await
    }

    async fn rls_policies(&self) -> Result<Vec<RlsPolicy>, StudioError> {
        apply(self.behavior, mock::rls_policies).await
    }

    async fn audit_entries(&self) -> Result<Vec<AuditEntry>, StudioError> {
        apply(self.behavior, mock::audit_entries).await
    }
}
