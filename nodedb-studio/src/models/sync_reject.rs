//! Rejected writes: a write a device made offline that the Origin refused
//! when it validated it. The direction is always device to Origin. The
//! Origin quarantines the write and sends the device a typed compensation
//! hint. Repair follows the hint. Nothing here picks a winner between two
//! versions of a record.

use nodedb_types::Value;
use serde::{Deserialize, Serialize};

use crate::models::feeds::Tone;

/// The constraint class the Origin reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RejectReason {
    UniqueViolation,
    ForeignKeyViolation,
    CheckConstraint,
    NotNullViolation,
}

impl RejectReason {
    pub const ALL: [RejectReason; 4] = [
        RejectReason::UniqueViolation,
        RejectReason::ForeignKeyViolation,
        RejectReason::CheckConstraint,
        RejectReason::NotNullViolation,
    ];

    /// The code the Origin sends, as the table shows it.
    pub fn label(self) -> &'static str {
        match self {
            RejectReason::UniqueViolation => "UNIQUE_VIOLATION",
            RejectReason::ForeignKeyViolation => "FOREIGN_KEY_VIOLATION",
            RejectReason::CheckConstraint => "CHECK_CONSTRAINT",
            RejectReason::NotNullViolation => "NOT_NULL_VIOLATION",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RejectState {
    /// Held at the Origin. It blocks no later write.
    Quarantined,
    /// A corrected write replaced it. The original stays on record.
    Resubmitted,
    /// The device was told to drop its pending copy.
    Discarded,
}

impl RejectState {
    pub const ALL: [RejectState; 3] = [
        RejectState::Quarantined,
        RejectState::Resubmitted,
        RejectState::Discarded,
    ];

    pub fn label(self) -> &'static str {
        match self {
            RejectState::Quarantined => "Quarantined",
            RejectState::Resubmitted => "Resubmitted",
            RejectState::Discarded => "Discarded",
        }
    }

    pub fn tone(self) -> Tone {
        match self {
            RejectState::Quarantined => Tone::Err,
            RejectState::Resubmitted => Tone::Ok,
            RejectState::Discarded => Tone::Idle,
        }
    }
}

/// What the Origin says would fix the write. It is a hint: the person
/// decides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hint {
    RetryWithDifferentValue {
        field: String,
        conflicting_value: String,
        suggestion: String,
    },
    CreateReferencedRow {
        collection: String,
        key: String,
    },
    ProvideRequiredField {
        field: String,
    },
    ManualIntervention {
        reason: String,
    },
}

impl Hint {
    pub fn title(&self) -> &'static str {
        match self {
            Hint::RetryWithDifferentValue { .. } => "Retry with a different value",
            Hint::CreateReferencedRow { .. } => "Create the referenced row first",
            Hint::ProvideRequiredField { .. } => "Provide the required field",
            Hint::ManualIntervention { .. } => "Manual intervention",
        }
    }

    pub fn text(&self) -> String {
        match self {
            Hint::RetryWithDifferentValue {
                field,
                conflicting_value,
                suggestion,
            } => format!("{field} {conflicting_value} exists at the Origin. Try {suggestion}."),
            Hint::CreateReferencedRow { collection, key } => {
                format!("{collection} {key} does not exist at the Origin.")
            }
            Hint::ProvideRequiredField { field } => format!("{field} must not be empty."),
            Hint::ManualIntervention { reason } => reason.clone(),
        }
    }
}

/// One stage of a rejection's repair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepState {
    Done,
    Failed,
    Waiting,
    Unknown,
}

impl StepState {
    pub fn label(self) -> &'static str {
        match self {
            StepState::Done => "Done",
            StepState::Failed => "Failed",
            StepState::Waiting => "Waiting",
            StepState::Unknown => "Unknown",
        }
    }

    pub fn tone(self) -> Tone {
        match self {
            StepState::Done => Tone::Ok,
            StepState::Failed => Tone::Err,
            StepState::Waiting | StepState::Unknown => Tone::Idle,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompensationStep {
    pub label: String,
    pub state: StepState,
    /// When the step happened, seconds since the epoch, if it did.
    pub at: Option<i64>,
    pub note: Option<String>,
}

/// Where the repair of one rejected write stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompensationState {
    pub mutation: String,
    pub steps: Vec<CompensationStep>,
    /// The new write that replaced it, once resubmitted.
    pub replacement: Option<String>,
    pub observed_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rejection {
    /// The mutation id, e.g. `m104`. Stable and unique per Origin.
    pub mutation: String,
    pub device_id: String,
    pub device_name: String,
    pub database: String,
    pub collection: String,
    pub reason: RejectReason,
    pub state: RejectState,
    /// The constraint the Origin checked, e.g. `orders_pkey (order_id)`.
    pub constraint: String,
    pub idempotency_key: String,
    /// When the device made the write, seconds since the epoch.
    pub device_time: i64,
    /// When the Origin received it.
    pub received_at: i64,
    /// The write with secrets and denied fields removed. A `Value::Object`.
    pub payload: Value,
    /// The key of the record the Origin already holds, for a key collision.
    pub existing: Option<String>,
    pub hint: Hint,
    /// Whether the cause is fixed at the Origin, so the same write can pass.
    pub corrected: bool,
    pub revision: u64,
}

impl Rejection {
    pub fn source(&self) -> String {
        format!("{}.{}", self.database, self.collection)
    }

    pub fn device_label(&self) -> String {
        format!("{} ({})", self.device_id, self.device_name)
    }

    pub fn matches(&self, needle: &str) -> bool {
        needle.is_empty()
            || [
                self.mutation.as_str(),
                self.device_id.as_str(),
                self.device_name.as_str(),
                self.collection.as_str(),
                self.reason.label(),
            ]
            .iter()
            .any(|f| f.to_lowercase().contains(needle))
    }

    /// Whether the person can still act on it.
    pub fn open(&self) -> bool {
        self.state == RejectState::Quarantined
    }

    /// Why Retry unchanged is unavailable, or `None` when it is.
    pub fn retry_blocked(&self) -> Option<String> {
        if !self.open() {
            Some(format!(
                "The write is {}.",
                self.state.label().to_lowercase()
            ))
        } else if !self.corrected {
            Some("Disabled until the error is corrected.".to_string())
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RejectedWrites {
    /// Newest first.
    pub rows: Vec<Rejection>,
    pub observed_at: i64,
}

impl RejectedWrites {
    #[cfg(any(test, all(feature = "screenshots", target_os = "linux")))]
    pub fn get(&self, mutation: &str) -> Option<&Rejection> {
        self.rows.iter().find(|r| r.mutation == mutation)
    }

    pub fn for_device(&self, device_id: &str) -> usize {
        self.rows
            .iter()
            .filter(|r| r.device_id == device_id && r.open())
            .count()
    }
}

/// What a person can ask of a rejected write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepairResult {
    pub operation: String,
    pub summary: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hint_says_what_the_origin_holds() {
        let h = Hint::RetryWithDifferentValue {
            field: "order_id".into(),
            conflicting_value: "o104".into(),
            suggestion: "o105".into(),
        };
        assert_eq!(h.text(), "order_id o104 exists at the Origin. Try o105.");
    }

    #[test]
    fn reason_codes_are_the_origin_words() {
        assert_eq!(RejectReason::UniqueViolation.label(), "UNIQUE_VIOLATION");
        assert_eq!(RejectReason::ALL.len(), 4);
    }
}
