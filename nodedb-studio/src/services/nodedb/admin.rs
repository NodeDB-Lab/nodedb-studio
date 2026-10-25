//! Administration reads.

use async_trait::async_trait;

use crate::models::admin::{AuditEntry, RlsPolicy, UserRow};
use crate::services::admin_data::AdminData;
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;

#[async_trait(?Send)]
impl AdminData for NodeDbConnectionService {
    async fn users(&self) -> Result<Vec<UserRow>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn rls_policies(&self) -> Result<Vec<RlsPolicy>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn audit_entries(&self) -> Result<Vec<AuditEntry>, StudioError> {
        Err(StudioError::NotConnected)
    }
}
