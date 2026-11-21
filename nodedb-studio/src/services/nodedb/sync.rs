//! Sync reads and operations.

use async_trait::async_trait;
use nodedb_types::Value;

use crate::models::cluster::op::OpResult;
use crate::models::sync::SyncDevices;
use crate::models::sync::activity::{SyncLog, SyncQuery};
use crate::models::sync::enroll::{EnrollOptions, EnrollRequest, Enrollment};
use crate::models::sync::reject::{CompensationState, RejectedWrites, RepairResult};
use crate::models::sync::sub::{ChangePreview, SubChange, Subscriptions};
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;
use crate::services::sync_data::SyncData;

/// NodeDB 0.6.2 keeps device, subscription and rejection state in server
/// memory with no statement or API to read or change it. Every method
/// refuses with the same wording. The Sync screen is hidden on this
/// backend, so a user does not normally see these.
fn unsupported<T>(action: &str) -> Result<T, StudioError> {
    Err(StudioError::Unsupported {
        what: format!(
            "Studio cannot {action}. NodeDB 0.6.2 has no statement or API that lists or manages Lite devices, subscriptions or rejected writes."
        ),
    })
}

#[async_trait(?Send)]
impl SyncData for NodeDbConnectionService {
    async fn sync_devices(&self) -> Result<SyncDevices, StudioError> {
        unsupported("list Lite devices")
    }

    async fn set_device_paused(
        &self,
        _id: &str,
        _paused: bool,
        _revision: u64,
    ) -> Result<OpResult, StudioError> {
        unsupported("pause or resume a Lite device")
    }

    async fn enroll_options(&self) -> Result<EnrollOptions, StudioError> {
        unsupported("offer enrollment choices for a Lite device")
    }

    async fn create_enrollment(&self, _request: &EnrollRequest) -> Result<Enrollment, StudioError> {
        unsupported("issue an enrollment token for a Lite device")
    }

    async fn enrollment_command(&self, _device_id: &str) -> Result<String, StudioError> {
        unsupported("build an enrollment command for a Lite device")
    }

    async fn sync_subscriptions(&self) -> Result<Subscriptions, StudioError> {
        unsupported("list Lite subscriptions")
    }

    async fn preview_subscription_change(
        &self,
        _id: &str,
        _change: &SubChange,
    ) -> Result<ChangePreview, StudioError> {
        unsupported("preview a change to a Lite subscription")
    }

    async fn apply_subscription_change(
        &self,
        _id: &str,
        _change: &SubChange,
        _revision: u64,
    ) -> Result<OpResult, StudioError> {
        unsupported("change a Lite subscription")
    }

    async fn set_subscription_paused(
        &self,
        _id: &str,
        _paused: bool,
        _revision: u64,
    ) -> Result<OpResult, StudioError> {
        unsupported("pause or resume a Lite subscription")
    }

    async fn rejected_writes(&self) -> Result<RejectedWrites, StudioError> {
        unsupported("list writes rejected from Lite devices")
    }

    async fn compensation_state(&self, _mutation: &str) -> Result<CompensationState, StudioError> {
        unsupported("read the repair state of a rejected write")
    }

    async fn retry_rejected(
        &self,
        _mutation: &str,
        _revision: u64,
    ) -> Result<RepairResult, StudioError> {
        unsupported("retry a rejected write")
    }

    async fn resubmit_rejected(
        &self,
        _mutation: &str,
        _payload: &Value,
        _revision: u64,
    ) -> Result<RepairResult, StudioError> {
        unsupported("resubmit a corrected write")
    }

    async fn discard_rejected(
        &self,
        _mutation: &str,
        _revision: u64,
    ) -> Result<RepairResult, StudioError> {
        unsupported("discard a rejected write")
    }

    async fn sync_activity(&self, _query: &SyncQuery) -> Result<SyncLog, StudioError> {
        unsupported("list sync activity")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::cluster::activity::ActivityRange;

    fn is_unsupported<T>(r: Result<T, StudioError>) -> bool {
        matches!(&r, Err(StudioError::Unsupported { what }) if what.contains("0.6.2"))
    }

    #[tokio::test]
    async fn every_method_is_unsupported() {
        let s = NodeDbConnectionService::default();
        let change = SubChange {
            filter: String::new(),
            parameters: Vec::new(),
        };
        let request = EnrollRequest {
            name: String::new(),
            database: String::new(),
            preset: String::new(),
            value: String::new(),
            role: String::new(),
        };
        let query = SyncQuery {
            range: ActivityRange::default(),
            device: None,
            kind: None,
        };
        assert!(is_unsupported(s.sync_devices().await));
        assert!(is_unsupported(s.set_device_paused("d", true, 1).await));
        assert!(is_unsupported(s.enroll_options().await));
        assert!(is_unsupported(s.create_enrollment(&request).await));
        assert!(is_unsupported(s.enrollment_command("d").await));
        assert!(is_unsupported(s.sync_subscriptions().await));
        assert!(is_unsupported(
            s.preview_subscription_change("s", &change).await
        ));
        assert!(is_unsupported(
            s.apply_subscription_change("s", &change, 1).await
        ));
        assert!(is_unsupported(
            s.set_subscription_paused("s", true, 1).await
        ));
        assert!(is_unsupported(s.rejected_writes().await));
        assert!(is_unsupported(s.compensation_state("m").await));
        assert!(is_unsupported(s.retry_rejected("m", 1).await));
        assert!(is_unsupported(
            s.resubmit_rejected("m", &Value::Null, 1).await
        ));
        assert!(is_unsupported(s.discard_rejected("m", 1).await));
        assert!(is_unsupported(s.sync_activity(&query).await));
    }
}
