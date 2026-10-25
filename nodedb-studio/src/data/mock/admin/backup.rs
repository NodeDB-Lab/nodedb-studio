//! Snapshots, the nightly schedule, and restores into new databases.

use crate::data::mock::admin::store::{AdminStore, DATABASE, missing};
use crate::models::admin::{AdminReview, ReviewLine, bytes_text};
use crate::models::admin_audit::TargetType;
use crate::models::admin_backup::{
    BackupChange, BackupOverview, Checksums, RestoreJob, RestoreState, Schedule, Snapshot,
    SnapshotState,
};
use crate::models::format::{clock_second, date_second, epoch};
use crate::services::error::StudioError;

const DESTINATION: &str = "s3://nodedb-backups/analytics";
const CREDENTIAL: &str = "backup_store";
const CIPHER: &str = "AES-256";

pub(super) fn seeded_schedule() -> Schedule {
    Schedule {
        name: "nightly-analytics".into(),
        database: DATABASE.into(),
        hour: 2,
        minute: 0,
        retention_days: 14,
        enabled: true,
    }
}

pub(super) fn seeded_snapshots() -> Vec<Snapshot> {
    vec![Snapshot {
        id: "snap-2026-06-13-020000-4f8c2d".into(),
        name: "nightly-analytics".into(),
        database: DATABASE.into(),
        schedule: "Daily 02:00 UTC".into(),
        state: SnapshotState::Completed,
        size_bytes: Some(18 << 30),
        cipher: Some(CIPHER.into()),
        retention_days: 14,
        created_at: epoch(2026, 6, 13, 2, 0, 0),
        checksums: Checksums::Verified,
        destination: DESTINATION.into(),
        credential: CREDENTIAL.into(),
        last_restore_test: None,
    }]
}

pub(super) fn seeded_restores() -> Vec<RestoreJob> {
    vec![RestoreJob {
        id: "restore103".into(),
        target: "analytics_restore".into(),
        snapshot: "snap-2026-06-12-020000-91b3e7".into(),
        state: RestoreState::Completed,
        created_at: epoch(2026, 6, 12, 3, 0, 0),
        duration_secs: Some(134),
    }]
}

/// The first and last restore point: one snapshot a night for a week.
fn restore_range() -> (i64, i64) {
    (epoch(2026, 6, 6, 2, 0, 0), epoch(2026, 6, 13, 2, 0, 0))
}

impl AdminStore {
    pub fn backup_overview(&self) -> BackupOverview {
        let (from, to) = restore_range();
        BackupOverview {
            snapshots: self.snapshots.clone(),
            restore_jobs: self.restores.clone(),
            schedule: self.schedule.clone(),
            databases: self.databases.clone(),
            restore_from: from,
            restore_to: to,
            observed_at: self.clock,
        }
    }

    pub(super) fn preview_backup(&self, change: &BackupChange) -> AdminReview {
        let mut review = AdminReview {
            title: String::new(),
            target: String::new(),
            lines: Vec::new(),
            notes: Vec::new(),
            warnings: Vec::new(),
            errors: Vec::new(),
            permission: self.permission("backup.create"),
            revision: self.rev.backups,
            apply_label: String::new(),
        };
        match change {
            BackupChange::CreateSnapshot { database } => {
                review.title = "Review snapshot".to_string();
                review.target = database.clone();
                review.apply_label = "Create snapshot".to_string();
                if !self.databases.contains(database) {
                    review.errors.push(format!("No database {database}."));
                }
                review.lines = vec![
                    ReviewLine::new("Snapshot", None, format!("On demand · {database}")),
                    ReviewLine::new("Storage destination", None, DESTINATION),
                    ReviewLine::new("Encrypted", None, format!("Yes ({CIPHER})")),
                    ReviewLine::new(
                        "Retention",
                        None,
                        format!("{} days", self.schedule.retention_days),
                    ),
                ];
                review.notes = vec![
                    "The snapshot is a consistent point in time. Writes continue while it runs."
                        .to_string(),
                    "A snapshot is a backup. It is not a replica of the live database.".to_string(),
                ];
            }
            BackupChange::SetSchedule(s) => {
                review.permission = self.permission("backup.schedule.write");
                review.title = "Review schedule".to_string();
                review.target = self.schedule.name.clone();
                review.apply_label = "Apply schedule".to_string();
                let cur = &self.schedule;
                if s.cadence() != cur.cadence() {
                    review.lines.push(ReviewLine::new(
                        "Schedule",
                        Some(&cur.cadence()),
                        s.cadence(),
                    ));
                }
                if s.retention_days != cur.retention_days {
                    review.lines.push(ReviewLine::new(
                        "Retention",
                        Some(&format!("{} days", cur.retention_days)),
                        format!("{} days", s.retention_days),
                    ));
                    if s.retention_days < cur.retention_days {
                        review.warnings.push(format!(
                            "Snapshots older than {} days are removed at the next run.",
                            s.retention_days
                        ));
                    }
                }
                if s.enabled != cur.enabled {
                    review.lines.push(ReviewLine::new(
                        "State",
                        Some(enabled(cur.enabled)),
                        enabled(s.enabled),
                    ));
                    if !s.enabled {
                        review.warnings.push(
                            "No snapshot is taken at night. The last one stays until its retention ends."
                                .to_string(),
                        );
                    }
                }
                if review.lines.is_empty() {
                    review
                        .errors
                        .push("The schedule has no change to apply.".to_string());
                }
                review.notes = vec!["Applying records the change in the audit trail.".to_string()];
            }
            BackupChange::Restore(r) => {
                review.permission = self.permission("backup.restore");
                review.title = "Review restore".to_string();
                review.target = r.target.clone();
                review.apply_label = "Start restore".to_string();
                let snap = self.snapshots.iter().find(|s| s.id == r.snapshot);
                let (from, to) = restore_range();
                match snap {
                    None => review.errors.push(format!("No snapshot {}.", r.snapshot)),
                    Some(s) if s.checksums == Checksums::Mismatch => review
                        .errors
                        .push("The snapshot failed its checksum check.".to_string()),
                    Some(s) if s.state != SnapshotState::Completed => review
                        .errors
                        .push("The snapshot has not finished.".to_string()),
                    Some(_) => {}
                }
                if r.point < from || r.point > to {
                    review
                        .errors
                        .push("The restore point is outside the available range.".to_string());
                }
                if let Some(issue) = (crate::models::admin_backup::RestoreDraft {
                    target: r.target.clone(),
                })
                .target_issue(&self.databases)
                {
                    review.errors.push(issue);
                }
                review.lines = vec![
                    ReviewLine::new("Source snapshot", None, r.snapshot.clone()),
                    ReviewLine::new("Restore point (UTC)", None, date_second(r.point, 0)),
                    ReviewLine::new("New database", None, r.target.clone()),
                    ReviewLine::new(
                        "Size to restore",
                        None,
                        snap.and_then(|s| s.size_bytes)
                            .map(bytes_text)
                            .unwrap_or_else(|| "Unknown".to_string()),
                    ),
                ];
                review.notes = vec![
                    "The restore creates a new database. It overwrites nothing.".to_string(),
                    "Replication does not restore data. The new database starts from the snapshot."
                        .to_string(),
                ];
            }
        }
        review
    }

    pub(super) fn apply_backup(&mut self, change: &BackupChange) -> Result<String, StudioError> {
        let summary = match change {
            BackupChange::CreateSnapshot { database } => {
                let stamp = clock_second(self.clock, 0).replace(':', "");
                let suffix = &super::audit::correlation(self.next_audit)[..6];
                let id = format!("snap-2026-06-13-{stamp}-{suffix}");
                self.snapshots.insert(
                    0,
                    Snapshot {
                        id: id.clone(),
                        name: format!("manual-{database}"),
                        database: database.clone(),
                        schedule: "On demand".into(),
                        state: SnapshotState::Running,
                        size_bytes: None,
                        cipher: Some(CIPHER.into()),
                        retention_days: self.schedule.retention_days,
                        created_at: self.clock,
                        checksums: Checksums::Pending,
                        destination: DESTINATION.into(),
                        credential: CREDENTIAL.into(),
                        last_restore_test: None,
                    },
                );
                let audit = self.record(
                    "Create snapshot",
                    TargetType::Snapshot,
                    &format!("snapshot:{id}"),
                    database,
                    vec!["Snapshots: stored".to_string()],
                    vec![format!("Snapshot {id}: running")],
                );
                format!("Started snapshot {id}. Recorded as {audit}.")
            }
            BackupChange::SetSchedule(s) => {
                let before = vec![
                    format!("Schedule: {}", self.schedule.cadence()),
                    format!("Retention: {} days", self.schedule.retention_days),
                    format!("State: {}", enabled(self.schedule.enabled)),
                ];
                self.schedule = s.clone();
                let after = vec![
                    format!("Schedule: {}", s.cadence()),
                    format!("Retention: {} days", s.retention_days),
                    format!("State: {}", enabled(s.enabled)),
                ];
                for snap in self.snapshots.iter_mut().filter(|n| n.name == s.name) {
                    snap.schedule = s.cadence();
                    snap.retention_days = s.retention_days;
                }
                let audit = self.record(
                    "Update schedule",
                    TargetType::Snapshot,
                    &format!("schedule:{}", s.name),
                    &s.database,
                    before,
                    after,
                );
                format!("Updated schedule {}. Recorded as {audit}.", s.name)
            }
            BackupChange::Restore(r) => {
                if !self.snapshots.iter().any(|s| s.id == r.snapshot) {
                    return Err(missing("snapshot", &r.snapshot));
                }
                let id = format!("restore{}", self.next_restore);
                self.next_restore += 1;
                self.restores.insert(
                    0,
                    RestoreJob {
                        id: id.clone(),
                        target: r.target.clone(),
                        snapshot: r.snapshot.clone(),
                        state: RestoreState::Running,
                        created_at: self.clock,
                        duration_secs: None,
                    },
                );
                // The name is taken from now on, even while the job runs.
                self.databases.push(r.target.clone());
                let audit = self.record(
                    "Restore snapshot",
                    TargetType::Snapshot,
                    &format!("snapshot:{}", r.snapshot),
                    &r.target,
                    vec!["Database: none".to_string()],
                    vec![format!("Database {}: restoring", r.target)],
                );
                format!(
                    "Started {id} into new database {}. Recorded as {audit}.",
                    r.target
                )
            }
        };
        self.rev.backups += 1;
        Ok(summary)
    }
}

fn enabled(on: bool) -> &'static str {
    if on { "Enabled" } else { "Disabled" }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::admin_backup::RestoreRequest;

    fn request(target: &str) -> BackupChange {
        BackupChange::Restore(RestoreRequest {
            snapshot: "snap-2026-06-13-020000-4f8c2d".into(),
            target: target.into(),
            point: epoch(2026, 6, 13, 2, 0, 0),
        })
    }

    #[test]
    fn a_restore_into_an_existing_database_cannot_apply() {
        let s = AdminStore::seeded();
        assert!(s.preview_backup(&request("analytics_restore")).can_apply());
        assert!(!s.preview_backup(&request("analytics")).can_apply());
    }

    #[test]
    fn the_snapshot_in_the_design_is_encrypted_and_untested() {
        let s = AdminStore::seeded();
        let o = s.backup_overview();
        let n = &o.snapshots[0];
        assert_eq!(n.size_bytes, Some(18 << 30));
        assert_eq!(n.last_restore_test, None);
        assert_eq!(n.checksums, Checksums::Verified);
    }
}
