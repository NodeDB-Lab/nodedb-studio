//! Sync activity: reconnects, bootstrap stages, acknowledgments, rejects,
//! and administrative changes, as the Origin currently observes them. It is
//! not a global audit guarantee: an event the Origin did not see is absent.

use serde::{Deserialize, Serialize};

use crate::models::feeds::Tone;
use crate::models::format::clock_second;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    OriginToLite,
    LiteToOrigin,
}

impl Direction {
    pub fn label(self) -> &'static str {
        match self {
            Direction::OriginToLite => "Origin → Lite",
            Direction::LiteToOrigin => "Lite → Origin",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncEventKind {
    Reconnected,
    Disconnected,
    SubscriptionStarted,
    SchemaAnnounced,
    SnapshotStarted,
    SnapshotComplete,
    BatchApplied,
    AckReceived,
    WriteAccepted,
    WriteRejected,
    DevicePaused,
    DeviceResumed,
    SubscriptionChanged,
    EnrollmentIssued,
    WriteDiscarded,
}

impl SyncEventKind {
    pub const ALL: [SyncEventKind; 15] = [
        SyncEventKind::Reconnected,
        SyncEventKind::Disconnected,
        SyncEventKind::SubscriptionStarted,
        SyncEventKind::SchemaAnnounced,
        SyncEventKind::SnapshotStarted,
        SyncEventKind::SnapshotComplete,
        SyncEventKind::BatchApplied,
        SyncEventKind::AckReceived,
        SyncEventKind::WriteAccepted,
        SyncEventKind::WriteRejected,
        SyncEventKind::DevicePaused,
        SyncEventKind::DeviceResumed,
        SyncEventKind::SubscriptionChanged,
        SyncEventKind::EnrollmentIssued,
        SyncEventKind::WriteDiscarded,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SyncEventKind::Reconnected => "Device reconnected",
            SyncEventKind::Disconnected => "Device disconnected",
            SyncEventKind::SubscriptionStarted => "Subscription started",
            SyncEventKind::SchemaAnnounced => "Schema announced",
            SyncEventKind::SnapshotStarted => "Snapshot started",
            SyncEventKind::SnapshotComplete => "Snapshot complete",
            SyncEventKind::BatchApplied => "Batch applied",
            SyncEventKind::AckReceived => "Acknowledgment received",
            SyncEventKind::WriteAccepted => "Write accepted",
            SyncEventKind::WriteRejected => "Write rejected",
            SyncEventKind::DevicePaused => "Device paused",
            SyncEventKind::DeviceResumed => "Device resumed",
            SyncEventKind::SubscriptionChanged => "Subscription changed",
            SyncEventKind::EnrollmentIssued => "Enrollment token issued",
            SyncEventKind::WriteDiscarded => "Write discarded",
        }
    }

    pub fn tone(self) -> Tone {
        match self {
            SyncEventKind::WriteRejected => Tone::Err,
            SyncEventKind::Disconnected | SyncEventKind::DevicePaused => Tone::Warn,
            SyncEventKind::Reconnected
            | SyncEventKind::SubscriptionStarted
            | SyncEventKind::SchemaAnnounced
            | SyncEventKind::SnapshotStarted
            | SyncEventKind::SnapshotComplete
            | SyncEventKind::BatchApplied
            | SyncEventKind::AckReceived
            | SyncEventKind::WriteAccepted
            | SyncEventKind::DeviceResumed
            | SyncEventKind::SubscriptionChanged
            | SyncEventKind::EnrollmentIssued
            | SyncEventKind::WriteDiscarded => Tone::Ok,
        }
    }

    /// An administrative change names who made it.
    pub fn administrative(self) -> bool {
        matches!(
            self,
            SyncEventKind::DevicePaused
                | SyncEventKind::DeviceResumed
                | SyncEventKind::SubscriptionChanged
                | SyncEventKind::EnrollmentIssued
                | SyncEventKind::WriteDiscarded
        )
    }
}

/// A position in a subscription's change stream and when it was reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor {
    pub position: u64,
    pub at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncEvent {
    /// Stable event id, e.g. `evt_s812`.
    pub id: String,
    /// When the Origin observed it, seconds since the epoch.
    pub at: i64,
    pub kind: SyncEventKind,
    pub device_id: String,
    pub device_name: String,
    pub database: String,
    /// The subscription name, when the event belongs to one.
    pub shape: Option<String>,
    pub direction: Option<Direction>,
    /// A short fact the table shows: a cursor or a write id.
    pub detail: Option<String>,
    pub received: Option<Cursor>,
    pub applied: Option<Cursor>,
    /// The device's acknowledgment. `None` is Not reported.
    pub acknowledged: Option<Cursor>,
    /// The rejected write's mutation id, for a reject.
    pub mutation: Option<String>,
    pub actor: Option<String>,
}

impl SyncEvent {
    pub fn matches(&self, needle: &str) -> bool {
        needle.is_empty()
            || [
                Some(self.id.as_str()),
                Some(self.kind.label()),
                Some(self.device_id.as_str()),
                Some(self.device_name.as_str()),
                self.shape.as_deref(),
                self.detail.as_deref(),
            ]
            .into_iter()
            .flatten()
            .any(|f| f.to_lowercase().contains(needle))
    }
}

/// What the server filters. The search text filters in Studio.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SyncQuery {
    pub range: crate::models::cluster_activity::ActivityRange,
    pub device: Option<String>,
    pub kind: Option<SyncEventKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncLog {
    pub observed_at: i64,
    /// Newest first.
    pub events: Vec<SyncEvent>,
}

/// `13:35:41` on the day of `reference`, else `06-12 09:41:03`.
pub fn stamp(at: i64, reference: i64) -> String {
    crate::models::cluster_activity::stamp(at, reference)
}

/// `13:39:47 UTC` for a cursor time.
pub fn utc(at: i64) -> String {
    format!("{} UTC", clock_second(at, 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_changes_by_a_person_are_administrative() {
        let admin: Vec<_> = SyncEventKind::ALL
            .iter()
            .filter(|k| k.administrative())
            .collect();
        assert_eq!(admin.len(), 5);
        assert!(!SyncEventKind::WriteRejected.administrative());
    }
}
