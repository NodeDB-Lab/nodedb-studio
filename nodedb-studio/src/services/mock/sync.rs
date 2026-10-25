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
use crate::services::mock::MockConnectionService;
use crate::services::mock_behavior::{MockBehavior, apply_one_result, apply_write};
use crate::services::sync_data::SyncData;

#[async_trait(?Send)]
impl SyncData for MockConnectionService {
    async fn sync_devices(&self) -> Result<SyncDevices, StudioError> {
        let mut read = self.sync.borrow().devices();
        if self.behavior == MockBehavior::Empty {
            read.devices.clear();
        }
        apply_one_result(self.behavior, Ok(read)).await
    }

    async fn set_device_paused(
        &self,
        id: &str,
        paused: bool,
        revision: u64,
    ) -> Result<OpResult, StudioError> {
        apply_write(self.behavior, || {
            self.sync
                .borrow_mut()
                .set_device_paused(id, paused, revision)
        })
        .await
    }

    async fn enroll_options(&self) -> Result<EnrollOptions, StudioError> {
        let read = self.sync.borrow().enroll_options();
        apply_one_result(self.behavior, Ok(read)).await
    }

    async fn create_enrollment(&self, request: &EnrollRequest) -> Result<Enrollment, StudioError> {
        apply_write(self.behavior, || self.sync.borrow_mut().enroll(request)).await
    }

    async fn enrollment_command(&self, device_id: &str) -> Result<String, StudioError> {
        apply_write(self.behavior, || {
            self.sync.borrow_mut().take_command(device_id)
        })
        .await
    }

    async fn sync_subscriptions(&self) -> Result<Subscriptions, StudioError> {
        let mut read = self.sync.borrow().subscriptions();
        if self.behavior == MockBehavior::Empty {
            read.items.clear();
        }
        apply_one_result(self.behavior, Ok(read)).await
    }

    async fn preview_subscription_change(
        &self,
        id: &str,
        change: &SubChange,
    ) -> Result<ChangePreview, StudioError> {
        let read = self.sync.borrow().preview_change(id, change);
        apply_one_result(self.behavior, read).await
    }

    async fn apply_subscription_change(
        &self,
        id: &str,
        change: &SubChange,
        revision: u64,
    ) -> Result<OpResult, StudioError> {
        apply_write(self.behavior, || {
            self.sync.borrow_mut().apply_change(id, change, revision)
        })
        .await
    }

    async fn set_subscription_paused(
        &self,
        id: &str,
        paused: bool,
        revision: u64,
    ) -> Result<OpResult, StudioError> {
        apply_write(self.behavior, || {
            self.sync
                .borrow_mut()
                .set_subscription_paused(id, paused, revision)
        })
        .await
    }

    async fn rejected_writes(&self) -> Result<RejectedWrites, StudioError> {
        let mut read = self.sync.borrow().rejected();
        if self.behavior == MockBehavior::Empty {
            read.rows.clear();
        }
        apply_one_result(self.behavior, Ok(read)).await
    }

    async fn compensation_state(&self, mutation: &str) -> Result<CompensationState, StudioError> {
        let read = self.sync.borrow().compensation(mutation);
        apply_one_result(self.behavior, read).await
    }

    async fn retry_rejected(
        &self,
        mutation: &str,
        revision: u64,
    ) -> Result<RepairResult, StudioError> {
        apply_write(self.behavior, || {
            self.sync.borrow_mut().retry_rejected(mutation, revision)
        })
        .await
    }

    async fn resubmit_rejected(
        &self,
        mutation: &str,
        payload: &Value,
        revision: u64,
    ) -> Result<RepairResult, StudioError> {
        apply_write(self.behavior, || {
            self.sync
                .borrow_mut()
                .resubmit_rejected(mutation, payload.clone(), revision)
        })
        .await
    }

    async fn discard_rejected(
        &self,
        mutation: &str,
        revision: u64,
    ) -> Result<RepairResult, StudioError> {
        apply_write(self.behavior, || {
            self.sync.borrow_mut().discard_rejected(mutation, revision)
        })
        .await
    }

    async fn sync_activity(&self, query: &SyncQuery) -> Result<SyncLog, StudioError> {
        let mut read = self.sync.borrow().activity(query);
        if self.behavior == MockBehavior::Empty {
            read.events.clear();
        }
        apply_one_result(self.behavior, Ok(read)).await
    }
}
