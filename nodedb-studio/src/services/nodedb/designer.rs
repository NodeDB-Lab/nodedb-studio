//! Schema design: staged changes, review, and apply.

use async_trait::async_trait;

use crate::models::designer::DesignSchema;
use crate::models::designer_change::{ApplyReport, ChangeKind, ChangeReview, PendingChange};
use crate::services::designer_data::DesignerData;
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;

#[async_trait(?Send)]
impl DesignerData for NodeDbConnectionService {
    async fn design_schema(&self) -> Result<DesignSchema, StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn pending_changes(&self) -> Result<Vec<PendingChange>, StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn stage_change(
        &self,
        _replace: Option<&str>,
        _kind: ChangeKind,
    ) -> Result<PendingChange, StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn discard_change(&self, _id: &str) -> Result<(), StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn discard_all_changes(&self) -> Result<(), StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn review_changes(&self) -> Result<ChangeReview, StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn apply_changes(&self, _revision: u64) -> Result<ApplyReport, StudioError> {
        Err(StudioError::NotConnected)
    }
}
