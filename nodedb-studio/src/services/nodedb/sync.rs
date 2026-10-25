//! Sync reads and operations.

use async_trait::async_trait;
use nodedb_types::Value;

use crate::models::cluster_op::OpResult;
use crate::models::sync::SyncDevices;
use crate::models::sync_activity::{SyncLog, SyncQuery};
use crate::models::sync_enroll::{EnrollOptions, EnrollRequest, Enrollment};
use crate::models::sync_reject::{CompensationState, RejectedWrites, RepairResult};
use crate::models::sync_sub::{ChangePreview, SubChange, Subscriptions};
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;
use crate::services::sync_data::SyncData;

#[async_trait(?Send)]
impl SyncData for NodeDbConnectionService {
    async fn sync_devices(&self) -> Result<SyncDevices, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn set_device_paused(
        &self,
        _id: &str,
        _paused: bool,
        _revision: u64,
    ) -> Result<OpResult, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn enroll_options(&self) -> Result<EnrollOptions, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn create_enrollment(&self, _request: &EnrollRequest) -> Result<Enrollment, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn enrollment_command(&self, _device_id: &str) -> Result<String, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn sync_subscriptions(&self) -> Result<Subscriptions, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn preview_subscription_change(
        &self,
        _id: &str,
        _change: &SubChange,
    ) -> Result<ChangePreview, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn apply_subscription_change(
        &self,
        _id: &str,
        _change: &SubChange,
        _revision: u64,
    ) -> Result<OpResult, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn set_subscription_paused(
        &self,
        _id: &str,
        _paused: bool,
        _revision: u64,
    ) -> Result<OpResult, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn rejected_writes(&self) -> Result<RejectedWrites, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn compensation_state(&self, _mutation: &str) -> Result<CompensationState, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn retry_rejected(
        &self,
        _mutation: &str,
        _revision: u64,
    ) -> Result<RepairResult, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn resubmit_rejected(
        &self,
        _mutation: &str,
        _payload: &Value,
        _revision: u64,
    ) -> Result<RepairResult, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn discard_rejected(
        &self,
        _mutation: &str,
        _revision: u64,
    ) -> Result<RepairResult, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn sync_activity(&self, _query: &SyncQuery) -> Result<SyncLog, StudioError> {
        Err(StudioError::NotConnected)
    }
}
