//! Sync reads and operations. Scope is the whole Origin cluster: none of
//! these depend on the selected database. A subscription change is read as
//! a preview first, and apply names the revision the preview read. A
//! rejected write is repaired through typed actions, never by picking a
//! winner between two versions of a record.

use async_trait::async_trait;
use nodedb_types::Value;

use crate::models::cluster_op::OpResult;
use crate::models::sync::SyncDevices;
use crate::models::sync_activity::{SyncLog, SyncQuery};
use crate::models::sync_enroll::{EnrollOptions, EnrollRequest, Enrollment};
use crate::models::sync_reject::{CompensationState, RejectedWrites, RepairResult};
use crate::models::sync_sub::{ChangePreview, SubChange, Subscriptions};
use crate::services::error::StudioError;

#[async_trait(?Send)]
pub trait SyncData {
    /// The Origin binding and every registered device with its reports.
    async fn sync_devices(&self) -> Result<SyncDevices, StudioError>;

    /// Pause or resume one device. `revision` is the one the list read.
    async fn set_device_paused(
        &self,
        id: &str,
        paused: bool,
        revision: u64,
    ) -> Result<OpResult, StudioError>;

    /// What the principal may offer a new device.
    async fn enroll_options(&self) -> Result<EnrollOptions, StudioError>;

    /// Issue a one-use enrollment token. The result carries it masked.
    async fn create_enrollment(&self, request: &EnrollRequest) -> Result<Enrollment, StudioError>;

    /// The command that carries the token. The server hands it out once.
    async fn enrollment_command(&self, device_id: &str) -> Result<String, StudioError>;

    /// Every subscription of every device.
    async fn sync_subscriptions(&self) -> Result<Subscriptions, StudioError>;

    /// What `change` would send and remove. Nothing changes.
    async fn preview_subscription_change(
        &self,
        id: &str,
        change: &SubChange,
    ) -> Result<ChangePreview, StudioError>;

    /// Apply a previewed change. `revision` is the preview's.
    async fn apply_subscription_change(
        &self,
        id: &str,
        change: &SubChange,
        revision: u64,
    ) -> Result<OpResult, StudioError>;

    /// Pause or resume one subscription. The device's others keep syncing.
    async fn set_subscription_paused(
        &self,
        id: &str,
        paused: bool,
        revision: u64,
    ) -> Result<OpResult, StudioError>;

    /// Every write the Origin rejected, newest first.
    async fn rejected_writes(&self) -> Result<RejectedWrites, StudioError>;

    /// Where the repair of one rejected write stands.
    async fn compensation_state(&self, mutation: &str) -> Result<CompensationState, StudioError>;

    /// Apply the write again, unchanged. Refused until the error is corrected.
    async fn retry_rejected(
        &self,
        mutation: &str,
        revision: u64,
    ) -> Result<RepairResult, StudioError>;

    /// Submit a corrected payload as a new write. The original stays on record.
    async fn resubmit_rejected(
        &self,
        mutation: &str,
        payload: &Value,
        revision: u64,
    ) -> Result<RepairResult, StudioError>;

    /// Tell the device to drop its pending copy. The audit record stays.
    async fn discard_rejected(
        &self,
        mutation: &str,
        revision: u64,
    ) -> Result<RepairResult, StudioError>;

    /// Events matching `query`, newest first.
    async fn sync_activity(&self, query: &SyncQuery) -> Result<SyncLog, StudioError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::sync::DeviceState;
    use crate::models::sync_reject::RejectState;
    use crate::services::async_state::AsyncState;
    use crate::services::mock::MockConnectionService;

    #[tokio::test]
    async fn device_ids_are_unique_and_every_state_appears() {
        let svc = MockConnectionService::ready();
        let list = svc.sync_devices().await.expect("devices");
        let mut ids: Vec<_> = list.devices.iter().map(|d| d.id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), list.devices.len());
        for s in DeviceState::ALL {
            assert!(list.count(s) > 0, "no {s:?} device");
        }
        assert_eq!(list.origin.cluster_id, "cl_a1");
    }

    #[tokio::test]
    async fn an_unreported_stage_stays_unknown() {
        let svc = MockConnectionService::ready();
        let list = svc.sync_devices().await.expect("devices");
        let offline = list.device("lite_003").expect("lite-kl-03");
        assert!(offline.pending.is_none());
        let live = list.device("lite_007").expect("lite-sg-07");
        assert_eq!(live.pending.map(|p| p.count), Some(28));
    }

    #[tokio::test]
    async fn every_subscription_belongs_to_a_registered_device() {
        let svc = MockConnectionService::ready();
        let devices = svc.sync_devices().await.expect("devices");
        let subs = svc.sync_subscriptions().await.expect("subs");
        let mut ids: Vec<_> = subs.items.iter().map(|s| s.id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), subs.items.len());
        for s in &subs.items {
            assert!(devices.device(&s.device_id).is_some(), "{}", s.id);
        }
    }

    #[tokio::test]
    async fn every_rejection_and_event_names_a_registered_device() {
        let svc = MockConnectionService::ready();
        let devices = svc.sync_devices().await.expect("devices");
        let rejected = svc.rejected_writes().await.expect("rejected");
        let mut ids: Vec<_> = rejected.rows.iter().map(|r| r.mutation.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), rejected.rows.len());
        for r in &rejected.rows {
            assert!(devices.device(&r.device_id).is_some(), "{}", r.mutation);
        }
        let log = svc.sync_activity(&SyncQuery::default()).await.expect("log");
        let mut ids: Vec<_> = log.events.iter().map(|e| e.id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), log.events.len());
        for e in &log.events {
            assert!(devices.device(&e.device_id).is_some(), "{}", e.id);
        }
    }

    #[tokio::test]
    async fn a_rejected_write_collides_with_the_record_the_explorer_holds() {
        use crate::services::explorer_data::ExplorerData;
        let svc = MockConnectionService::ready();
        let rejected = svc.rejected_writes().await.expect("rejected");
        let m104 = rejected.get("m104").expect("m104");
        let key = m104.existing.clone().expect("existing record");
        let stored = svc
            .record(&m104.collection, &key)
            .await
            .expect("explorer record");
        assert_eq!(stored.id, "o104");
    }

    #[tokio::test]
    async fn pausing_a_device_shows_in_the_list_and_the_log() {
        let svc = MockConnectionService::ready();
        let list = svc.sync_devices().await.expect("devices");
        let d = list.device("d17").expect("d17");
        svc.set_device_paused("d17", true, d.revision)
            .await
            .expect("pause");
        let after = svc.sync_devices().await.expect("devices");
        assert_eq!(
            after.device("d17").map(|d| d.state),
            Some(DeviceState::Paused)
        );
        let log = svc.sync_activity(&SyncQuery::default()).await.expect("log");
        assert_eq!(
            log.events[0].kind,
            crate::models::sync_activity::SyncEventKind::DevicePaused
        );
        // The old revision no longer applies.
        let err = svc.set_device_paused("d17", false, d.revision).await;
        assert!(matches!(err, Err(StudioError::StaleRevision { .. })));
    }

    #[tokio::test]
    async fn a_changed_subscription_previews_then_resnapshots() {
        let svc = MockConnectionService::ready();
        let subs = svc.sync_subscriptions().await.expect("subs");
        let s = subs.get("sub_17_orders").expect("orders_for_store");
        let mut change = s.as_change();
        let same = svc
            .preview_subscription_change(&s.id, &change)
            .await
            .expect("preview");
        assert!(same.unchanged() && !same.can_apply());
        change.parameters[0].1 = "sf".into();
        let p = svc
            .preview_subscription_change(&s.id, &change)
            .await
            .expect("preview");
        assert!(p.resnapshot && p.can_apply());
        assert_eq!(p.current.rows, 128_431);
        assert_eq!(p.proposed.rows, 96_204);
        // Nothing changed until apply.
        let unchanged = svc.sync_subscriptions().await.expect("subs");
        assert_eq!(unchanged.get(&s.id).map(|s| s.revision), Some(s.revision));
        svc.apply_subscription_change(&s.id, &change, p.revision)
            .await
            .expect("apply");
        let after = svc.sync_subscriptions().await.expect("subs");
        let now = after.get(&s.id).expect("sub");
        assert_eq!(now.parameters[0].1, "sf");
        assert!(now.revision > s.revision);
        let stale = svc
            .apply_subscription_change(&s.id, &change, p.revision)
            .await;
        assert!(matches!(stale, Err(StudioError::StaleRevision { .. })));
    }

    #[tokio::test]
    async fn a_principal_without_the_privilege_cannot_apply() {
        let svc = MockConnectionService::ready().without_sync_permission();
        let subs = svc.sync_subscriptions().await.expect("subs");
        let s = subs.get("sub_17_orders").expect("sub");
        let mut change = s.as_change();
        change.parameters[0].1 = "la".into();
        let p = svc
            .preview_subscription_change(&s.id, &change)
            .await
            .expect("preview");
        assert!(!p.can_apply());
        let r = svc
            .apply_subscription_change(&s.id, &change, p.revision)
            .await;
        assert!(matches!(r, Err(StudioError::PermissionDenied { .. })));
    }

    #[tokio::test]
    async fn retry_waits_for_the_correction_and_resubmit_needs_a_real_edit() {
        let svc = MockConnectionService::ready();
        let rows = svc.rejected_writes().await.expect("rejected");
        let m104 = rows.get("m104").expect("m104").clone();
        assert!(m104.retry_blocked().is_some());
        let retry = svc.retry_rejected("m104", m104.revision).await;
        assert!(matches!(retry, Err(StudioError::Rejected { .. })));
        // The same payload is not a correction.
        let same = svc
            .resubmit_rejected("m104", &m104.payload, m104.revision)
            .await;
        assert!(matches!(same, Err(StudioError::Rejected { .. })));
        // An id the Origin holds is still a unique violation.
        let mut clash = m104.payload.clone();
        if let Value::Object(map) = &mut clash {
            map.insert("order_id".into(), Value::String("o103".into()));
        }
        assert!(
            svc.resubmit_rejected("m104", &clash, m104.revision)
                .await
                .is_err()
        );
        let mut fixed = m104.payload.clone();
        if let Value::Object(map) = &mut fixed {
            map.insert("order_id".into(), Value::String("o105".into()));
        }
        svc.resubmit_rejected("m104", &fixed, m104.revision)
            .await
            .expect("resubmit");
        let after = svc.rejected_writes().await.expect("rejected");
        assert_eq!(
            after.get("m104").map(|r| r.state),
            Some(RejectState::Resubmitted)
        );
        let comp = svc.compensation_state("m104").await.expect("compensation");
        assert_eq!(comp.replacement.as_deref(), Some("m104-r1"));
    }

    #[tokio::test]
    async fn a_corrected_write_retries_unchanged_and_a_discard_keeps_the_audit() {
        let svc = MockConnectionService::ready();
        let rows = svc.rejected_writes().await.expect("rejected");
        let m103 = rows.get("m103").expect("m103").clone();
        assert!(m103.retry_blocked().is_none());
        svc.retry_rejected("m103", m103.revision)
            .await
            .expect("retry");
        let m102 = rows.get("m102").expect("m102").clone();
        svc.discard_rejected("m102", m102.revision)
            .await
            .expect("discard");
        let after = svc.rejected_writes().await.expect("rejected");
        assert_eq!(
            after.get("m102").map(|r| r.state),
            Some(RejectState::Discarded)
        );
        assert!(after.get("m102").is_some(), "the audit record stays");
        let again = svc.discard_rejected("m102", m102.revision + 1).await;
        assert!(matches!(again, Err(StudioError::Rejected { .. })));
    }

    #[tokio::test]
    async fn an_enrollment_token_is_masked_and_its_command_is_handed_out_once() {
        let svc = MockConnectionService::ready();
        let options = svc.enroll_options().await.expect("options");
        assert_eq!(options.device_id, "d18");
        let req = EnrollRequest {
            name: "Checkout iPad 2".into(),
            database: "analytics".into(),
            preset: "store_orders".into(),
            value: "nyc".into(),
            role: "sync_checkout".into(),
        };
        let issued = svc.create_enrollment(&req).await.expect("enrollment");
        assert_eq!(issued.device_id, "d18");
        assert!(issued.masked.contains('•') && !issued.masked.contains("enr_d18"));
        assert_eq!(issued.expires_at - issued.issued_at, 900);
        let command = svc.enrollment_command("d18").await.expect("command");
        assert!(command.contains("--token enr_"));
        assert!(svc.enrollment_command("d18").await.is_err());
        // The next device gets the next id.
        assert_eq!(
            svc.enroll_options().await.expect("options").device_id,
            "d19"
        );
        // A name in use is refused.
        let dup = EnrollRequest {
            name: "checkout-ipad".into(),
            ..req
        };
        assert!(svc.create_enrollment(&dup).await.is_err());
    }

    #[tokio::test]
    async fn empty_and_error_states_are_reachable() {
        let svc = MockConnectionService::empty();
        let s = AsyncState::from_value(Some(svc.sync_devices().await));
        assert!(s.is_empty());
        let s = AsyncState::from_value(Some(svc.sync_subscriptions().await));
        assert!(s.is_empty());
        let s = AsyncState::from_value(Some(svc.rejected_writes().await));
        assert!(s.is_empty());
        let s = AsyncState::from_value(Some(svc.sync_activity(&SyncQuery::default()).await));
        assert!(s.is_empty());
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.sync_devices().await));
        assert!(s.error_message().is_some());
        assert!(svc.rejected_writes().await.is_err());
    }
}
