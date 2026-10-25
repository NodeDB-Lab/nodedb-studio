//! Repairs of rejected writes. Each one follows the typed rejection and
//! leaves the original on record: a correction never replaces the rejected
//! user data in place.

use nodedb_types::Value;

use crate::data::mock::explorer::{Overlay, stored_record};
use crate::data::mock::sync::store::{SyncStore, missing, stale};
use crate::models::sync_activity::{Direction, SyncEventKind};
use crate::models::sync_reject::{
    CompensationState, CompensationStep, Hint, RejectReason, RejectState, Rejection, RepairResult,
    StepState,
};
use crate::services::error::StudioError;

fn refused(reason: impl Into<String>) -> StudioError {
    StudioError::Rejected {
        reason: reason.into(),
    }
}

fn text_of(payload: &Value, field: &str) -> Option<String> {
    match payload {
        Value::Object(map) => match map.get(field)? {
            Value::String(s) => Some(s.clone()),
            Value::Null => None,
            other => Some(format!("{other:?}")),
        },
        _ => None,
    }
}

impl SyncStore {
    fn rejection_mut(
        &mut self,
        mutation: &str,
        revision: u64,
    ) -> Result<&mut Rejection, StudioError> {
        let r = self
            .rejects
            .iter_mut()
            .find(|r| r.mutation == mutation)
            .ok_or_else(|| missing("rejected write", mutation))?;
        if r.revision != revision {
            return Err(stale(
                format!("rejected write {mutation}"),
                revision,
                r.revision,
            ));
        }
        if !r.open() {
            return Err(refused(format!(
                "{mutation} is {}. Only a quarantined write takes a repair.",
                r.state.label().to_lowercase()
            )));
        }
        Ok(r)
    }

    fn step(&mut self, mutation: &str, label: &str, state: StepState, note: Option<String>) {
        let at = self.clock;
        self.repairs.push((
            mutation.to_string(),
            CompensationStep {
                label: label.to_string(),
                state,
                at: Some(at),
                note,
            },
        ));
    }

    fn log_write(&mut self, kind: SyncEventKind, r: &Rejection, detail: String) {
        let mut e = Self::event(kind, (&r.device_id, &r.device_name), None, Some(detail));
        e.direction = Some(Direction::LiteToOrigin);
        e.mutation = Some(r.mutation.clone());
        self.record(e);
    }

    /// Where the repair of `mutation` stands.
    pub fn compensation(&self, mutation: &str) -> Result<CompensationState, StudioError> {
        let r = self
            .rejects
            .iter()
            .find(|r| r.mutation == mutation)
            .ok_or_else(|| missing("rejected write", mutation))?;
        let done = |label: String, at: i64, note: Option<String>| CompensationStep {
            label,
            state: StepState::Done,
            at: Some(at),
            note,
        };
        let mut steps = vec![
            done(
                format!("Write received from {}", r.device_name),
                r.received_at,
                Some(format!(
                    "Made on the device at {}.",
                    crate::models::sync_activity::utc(r.device_time)
                )),
            ),
            CompensationStep {
                label: format!("Checked against {}", r.constraint),
                state: StepState::Failed,
                at: Some(r.received_at),
                note: Some(r.reason.label().to_string()),
            },
            done(
                "Quarantined at the Origin".to_string(),
                r.received_at,
                Some("Later writes from the device continue to apply.".to_string()),
            ),
            done(
                format!("Reject and hint sent to {}", r.device_name),
                r.received_at + 1,
                Some(r.hint.title().to_string()),
            ),
            CompensationStep {
                label: "Device acknowledged the reject".to_string(),
                state: StepState::Unknown,
                at: None,
                note: Some("Not reported".to_string()),
            },
        ];
        let extra: Vec<CompensationStep> = self
            .repairs
            .iter()
            .filter(|(m, _)| m == mutation)
            .map(|(_, s)| s.clone())
            .collect();
        if extra.is_empty() && r.open() {
            steps.push(CompensationStep {
                label: "Repair".to_string(),
                state: StepState::Waiting,
                at: None,
                note: Some("Waiting for a correction or a discard.".to_string()),
            });
        }
        steps.extend(extra);
        Ok(CompensationState {
            mutation: mutation.to_string(),
            steps,
            replacement: self
                .replacements
                .iter()
                .find(|(m, _)| m == mutation)
                .map(|(_, n)| n.clone()),
            observed_at: self.clock,
        })
    }

    /// Apply the same write again. The Origin runs its checks again.
    pub fn retry_rejected(
        &mut self,
        mutation: &str,
        revision: u64,
    ) -> Result<RepairResult, StudioError> {
        let r = self.rejection_mut(mutation, revision)?;
        if !r.corrected {
            return Err(refused(
                "The error is not corrected. Retrying the same write fails the same check.",
            ));
        }
        r.state = RejectState::Resubmitted;
        r.revision += 1;
        let r = r.clone();
        self.step(
            mutation,
            "Retried unchanged",
            StepState::Done,
            Some("The write passed its checks.".to_string()),
        );
        self.log_write(
            SyncEventKind::WriteAccepted,
            &r,
            format!("write {mutation} (retry)"),
        );
        let operation = self.operation();
        Ok(RepairResult {
            operation,
            summary: format!("Applied {mutation} from {} unchanged.", r.device_name),
        })
    }

    /// Submit a corrected payload as a new write. The original stays.
    pub fn resubmit_rejected(
        &mut self,
        mutation: &str,
        payload: Value,
        revision: u64,
    ) -> Result<RepairResult, StudioError> {
        let r = self.rejection_mut(mutation, revision)?;
        if payload == r.payload {
            return Err(refused(
                "The payload is unchanged. Edit it to correct the error, or use Retry unchanged once the Origin is corrected.",
            ));
        }
        match (&r.reason, &r.hint) {
            (RejectReason::UniqueViolation, Hint::RetryWithDifferentValue { field, .. }) => {
                let value = text_of(&payload, field)
                    .ok_or_else(|| refused(format!("{field} is required.")))?;
                if stored_record(&r.collection, &value, &Overlay::default()).is_some() {
                    return Err(refused(format!(
                        "UNIQUE_VIOLATION on {}: {field} {value} exists at the Origin.",
                        r.constraint
                    )));
                }
            }
            (RejectReason::NotNullViolation, Hint::ProvideRequiredField { field }) => {
                if text_of(&payload, field).is_none_or(|v| v.trim().is_empty()) {
                    return Err(refused(format!(
                        "NOT_NULL_VIOLATION: {field} must not be empty."
                    )));
                }
            }
            (RejectReason::CheckConstraint, _) => {
                let total = text_of(&payload, "total").and_then(|t| t.parse::<f64>().ok());
                if total.is_none_or(|t| t <= 0.0) {
                    return Err(refused(
                        "CHECK_CONSTRAINT on orders_total_positive: total must be greater than 0.",
                    ));
                }
            }
            (RejectReason::ForeignKeyViolation, _) if !r.corrected => {
                return Err(refused(
                    "FOREIGN_KEY_VIOLATION: the referenced row still does not exist at the Origin.",
                ));
            }
            _ => {}
        }
        let new = format!("{mutation}-r1");
        r.state = RejectState::Resubmitted;
        r.revision += 1;
        let r = r.clone();
        self.replacements.push((mutation.to_string(), new.clone()));
        self.step(
            mutation,
            &format!("Resubmitted as {new}"),
            StepState::Done,
            Some(
                "The corrected write passed its checks. The rejected payload stays on record."
                    .to_string(),
            ),
        );
        self.log_write(SyncEventKind::WriteAccepted, &r, format!("write {new}"));
        let operation = self.operation();
        Ok(RepairResult {
            operation,
            summary: format!(
                "Accepted {new} from {}. {mutation} stays on record.",
                r.device_name
            ),
        })
    }

    /// Tell the device to drop its pending copy. The Origin keeps the audit.
    pub fn discard_rejected(
        &mut self,
        mutation: &str,
        revision: u64,
    ) -> Result<RepairResult, StudioError> {
        let r = self.rejection_mut(mutation, revision)?;
        r.state = RejectState::Discarded;
        r.revision += 1;
        let r = r.clone();
        self.step(
            mutation,
            &format!("Discard sent to {}", r.device_name),
            StepState::Done,
            Some("The device drops its pending copy when it next reports.".to_string()),
        );
        let mut e = Self::event(
            SyncEventKind::WriteDiscarded,
            (&r.device_id, &r.device_name),
            None,
            Some(format!("write {mutation}")),
        );
        e.mutation = Some(mutation.to_string());
        self.record(e);
        let operation = self.operation();
        Ok(RepairResult {
            operation,
            summary: format!(
                "Discarded {mutation} on {}. The audit record stays.",
                r.device_name
            ),
        })
    }
}
