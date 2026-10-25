//! Import and export jobs.

use async_trait::async_trait;

use crate::models::transfer::{ExportPreview, ExportRequest, TransferJob};
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;
use crate::services::transfer_data::TransferData;

#[async_trait(?Send)]
impl TransferData for NodeDbConnectionService {
    async fn transfer_jobs(&self) -> Result<Vec<TransferJob>, StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn export_preview(&self, _request: &ExportRequest) -> Result<ExportPreview, StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn start_export(&self, _request: &ExportRequest) -> Result<TransferJob, StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn cancel_job(&self, _id: &str) -> Result<(), StudioError> {
        Err(StudioError::NotConnected)
    }
}
