//! Backups: scheduled snapshots, restore jobs, and the restore preview.
//!
//! A snapshot is a consistent point-in-time copy kept for recovery.
//! Replication keeps live replicas in step and copies a mistaken write to
//! every replica, so it is no backup. The two never share a control here.
//! A restore creates a new database. Restoring over an existing database is
//! a separate review that is not designed yet.

use crate::models::admin::identifier_issue;
use crate::models::feeds::Tone;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotState {
    Completed,
    Running,
    // The server reports it. No fixture holds a failed snapshot.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "services::nodedb::admin builds it from a failed server snapshot; that service is a stub today"
        )
    )]
    Failed,
}

impl SnapshotState {
    pub fn label(self) -> &'static str {
        match self {
            SnapshotState::Completed => "Completed",
            SnapshotState::Running => "Running",
            SnapshotState::Failed => "Failed",
        }
    }

    pub fn tone(self) -> Tone {
        match self {
            SnapshotState::Completed => Tone::Ok,
            SnapshotState::Running => Tone::Warn,
            SnapshotState::Failed => Tone::Err,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Checksums {
    Verified,
    /// The snapshot has not finished, so there is nothing to check yet.
    Pending,
    Mismatch,
}

impl Checksums {
    pub fn label(self) -> &'static str {
        match self {
            Checksums::Verified => "Verified",
            Checksums::Pending => "Pending",
            Checksums::Mismatch => "Mismatch",
        }
    }

    pub fn tone(self) -> Tone {
        match self {
            Checksums::Verified => Tone::Ok,
            Checksums::Pending => Tone::Idle,
            Checksums::Mismatch => Tone::Err,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// The consistent-point id, e.g. `snap-2026-06-13-020000-4f8c2d`.
    pub id: String,
    /// The schedule that took it, or the on-demand name.
    pub name: String,
    pub database: String,
    /// `Daily 02:00 UTC`, or `On demand`.
    pub schedule: String,
    pub state: SnapshotState,
    /// Unknown until the snapshot finishes.
    pub size_bytes: Option<u64>,
    /// The cipher, or `None` for an unencrypted snapshot.
    pub cipher: Option<String>,
    pub retention_days: u32,
    pub created_at: i64,
    pub checksums: Checksums,
    /// Where the snapshot is stored. It holds no secret.
    pub destination: String,
    /// The credential alias the server uses to reach the destination.
    pub credential: String,
    /// When a restore of this snapshot last passed a test. `None` is Not reported.
    pub last_restore_test: Option<i64>,
}

impl Snapshot {
    pub fn is_scheduled(&self) -> bool {
        self.schedule != "On demand"
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreState {
    Completed,
    Running,
    // The server reports it. No fixture holds a failed restore.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "services::nodedb::admin builds it from a failed server restore; that service is a stub today"
        )
    )]
    Failed,
}

impl RestoreState {
    pub fn label(self) -> &'static str {
        match self {
            RestoreState::Completed => "Completed",
            RestoreState::Running => "Running",
            RestoreState::Failed => "Failed",
        }
    }

    pub fn tone(self) -> Tone {
        match self {
            RestoreState::Completed => Tone::Ok,
            RestoreState::Running => Tone::Warn,
            RestoreState::Failed => Tone::Err,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreJob {
    pub id: String,
    /// The new database the restore created.
    pub target: String,
    pub snapshot: String,
    pub state: RestoreState,
    pub created_at: i64,
    /// Unknown while the job runs.
    pub duration_secs: Option<u64>,
}

/// The schedule that takes the nightly snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schedule {
    pub name: String,
    pub database: String,
    pub hour: u32,
    pub minute: u32,
    pub retention_days: u32,
    pub enabled: bool,
}

impl Schedule {
    /// `Daily 02:00 UTC`.
    pub fn cadence(&self) -> String {
        format!("Daily {:02}:{:02} UTC", self.hour, self.minute)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupOverview {
    pub snapshots: Vec<Snapshot>,
    pub restore_jobs: Vec<RestoreJob>,
    pub schedule: Schedule,
    /// Databases that exist now. A restore cannot take one of their names.
    pub databases: Vec<String>,
    /// The first and last point a restore can use, seconds since the epoch.
    pub restore_from: i64,
    pub restore_to: i64,
    pub observed_at: i64,
}

/// What the person typed for the restore preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreDraft {
    pub target: String,
}

impl RestoreDraft {
    pub fn target_issue(&self, existing: &[String]) -> Option<String> {
        identifier_issue(&self.target, "database").or_else(|| {
            existing.iter().any(|d| *d == self.target.trim()).then(|| {
                format!(
                    "`{}` exists. Restoring over an existing database needs its own review, which is not designed yet.",
                    self.target.trim()
                )
            })
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreRequest {
    pub snapshot: String,
    pub target: String,
    /// The restore point, seconds since the epoch.
    pub point: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackupChange {
    CreateSnapshot { database: String },
    SetSchedule(Schedule),
    Restore(RestoreRequest),
}

/// What the schedule form edits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleDraft {
    pub hour: u32,
    pub minute: u32,
    pub retention_days: u32,
    pub enabled: bool,
}

impl ScheduleDraft {
    pub fn of(s: &Schedule) -> Self {
        Self {
            hour: s.hour,
            minute: s.minute,
            retention_days: s.retention_days,
            enabled: s.enabled,
        }
    }

    pub fn schedule(&self, base: &Schedule) -> Schedule {
        Schedule {
            hour: self.hour,
            minute: self.minute,
            retention_days: self.retention_days,
            enabled: self.enabled,
            ..base.clone()
        }
    }
}

/// How many days of snapshots a schedule may keep.
pub const RETENTION_CHOICES: [u32; 5] = [7, 14, 30, 60, 90];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_snapshot_or_restore_reads_as_an_error() {
        assert_eq!(SnapshotState::Failed.label(), "Failed");
        assert_eq!(SnapshotState::Failed.tone(), Tone::Err);
        assert_eq!(RestoreState::Failed.label(), "Failed");
        assert_eq!(RestoreState::Failed.tone(), Tone::Err);
    }

    #[test]
    fn a_restore_target_must_be_new() {
        let existing = vec!["analytics".to_string(), "events_log".to_string()];
        let d = |t: &str| RestoreDraft { target: t.into() };
        assert!(d("analytics_restore").target_issue(&existing).is_none());
        assert!(d("analytics").target_issue(&existing).is_some());
        assert!(d("").target_issue(&existing).is_some());
        assert!(d("Bad Name").target_issue(&existing).is_some());
    }

    #[test]
    fn a_cadence_reads_in_utc() {
        let s = Schedule {
            name: "nightly-analytics".into(),
            database: "analytics".into(),
            hour: 2,
            minute: 0,
            retention_days: 14,
            enabled: true,
        };
        assert_eq!(s.cadence(), "Daily 02:00 UTC");
    }
}
