//! Scheduled jobs: named SQL that the server runs on a cron schedule.
//!
//! A schedule is not a stream. A job runs, ends, and leaves a run in its
//! history. Run now, Disable, and Cancel run are separate operations on a
//! job or on one run.

use serde::{Deserialize, Serialize};

use crate::models::cron::{CronError, Schedule};
use crate::models::feeds::Tone;
use crate::models::zone::Zone;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OverlapPolicy {
    /// Start no new run while one is running.
    Skip,
    /// Start the new run when the current one ends.
    Queue,
    /// Run in parallel.
    Allow,
}

impl OverlapPolicy {
    pub const ALL: [OverlapPolicy; 3] = [
        OverlapPolicy::Skip,
        OverlapPolicy::Queue,
        OverlapPolicy::Allow,
    ];

    pub fn label(self) -> &'static str {
        match self {
            OverlapPolicy::Skip => "Skip",
            OverlapPolicy::Queue => "Queue",
            OverlapPolicy::Allow => "Allow",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            OverlapPolicy::Skip => "Start no new run while one is running",
            OverlapPolicy::Queue => "Start the new run when the current one ends",
            OverlapPolicy::Allow => "Run in parallel",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunState {
    Running,
    Success,
    Failed,
    Cancelled,
}

impl RunState {
    pub fn label(self) -> &'static str {
        match self {
            RunState::Running => "Running",
            RunState::Success => "Success",
            RunState::Failed => "Failed",
            RunState::Cancelled => "Cancelled",
        }
    }

    pub fn tone(self) -> Tone {
        match self {
            RunState::Running => Tone::Warn,
            RunState::Success => Tone::Ok,
            RunState::Failed => Tone::Err,
            RunState::Cancelled => Tone::Idle,
        }
    }
}

/// Timeouts a job can use, in seconds.
pub const TIMEOUTS: [u32; 6] = [30, 60, 300, 900, 1800, 3600];
/// Backoff intervals between retries, in seconds.
pub const BACKOFFS: [u32; 4] = [10, 30, 60, 300];

/// `30 s`, `5 min`, `1 h`.
pub fn secs_label(secs: u32) -> String {
    match secs {
        s if s >= 3600 && s % 3600 == 0 => format!("{} h", s / 3600),
        s if s >= 60 && s % 60 == 0 => format!("{} min", s / 60),
        s => format!("{s} s"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledJob {
    /// Stable identity.
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub cron: String,
    /// An IANA name from `Zone`.
    pub timezone: String,
    /// The collection the SQL reads, as `database.collection`.
    pub source: String,
    pub sql: String,
    /// The role the SQL runs as.
    pub owner: String,
    pub timeout_secs: u32,
    pub overlap: OverlapPolicy,
    pub retries: u32,
    pub backoff_secs: u32,
    /// Changes with every edit. A save names the revision it read.
    pub revision: u64,
}

/// The jobs of a database and when the server was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobList {
    /// Seconds since the epoch, UTC. Next runs count from here.
    pub observed_at: i64,
    pub jobs: Vec<ScheduledJob>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobRun {
    pub id: String,
    pub job: String,
    /// `2026-06-13 02:00:00`, UTC.
    pub started: String,
    pub state: RunState,
    pub duration_secs: Option<u64>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogLine {
    pub at: String,
    pub text: String,
}

/// A field of the job form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobField {
    Name,
    Schedule,
    Timezone,
    Source,
    Sql,
    Owner,
    Retries,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobIssue {
    pub at: JobField,
    pub message: String,
}

/// The job editor's fields. It edits an existing job, or a new one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobForm {
    pub name: String,
    pub cron: String,
    pub timezone: String,
    pub source: String,
    pub sql: String,
    pub owner: String,
    pub timeout_secs: u32,
    pub overlap: OverlapPolicy,
    pub retries: String,
    pub backoff_secs: u32,
}

impl JobForm {
    pub fn blank(source: &str, owner: &str) -> Self {
        Self {
            name: String::new(),
            cron: "0 2 * * *".into(),
            timezone: Zone::Utc.name().into(),
            source: source.to_string(),
            sql: String::new(),
            owner: owner.to_string(),
            timeout_secs: 300,
            overlap: OverlapPolicy::Skip,
            retries: "3".into(),
            backoff_secs: 30,
        }
    }

    pub fn of(job: &ScheduledJob) -> Self {
        Self {
            name: job.name.clone(),
            cron: job.cron.clone(),
            timezone: job.timezone.clone(),
            source: job.source.clone(),
            sql: job.sql.clone(),
            owner: job.owner.clone(),
            timeout_secs: job.timeout_secs,
            overlap: job.overlap,
            retries: job.retries.to_string(),
            backoff_secs: job.backoff_secs,
        }
    }

    pub fn retries_value(&self) -> Option<u32> {
        self.retries.trim().parse().ok().filter(|n| *n <= 10)
    }

    pub fn zone(&self) -> Option<Zone> {
        Zone::from_name(&self.timezone)
    }

    /// The next `count` runs after `now`, as UTC instants.
    pub fn next_runs(&self, now: i64, count: usize) -> Result<Vec<i64>, CronError> {
        let schedule = Schedule::parse(&self.cron)?;
        Ok(schedule.next_runs(now, self.zone().unwrap_or(Zone::Utc), count))
    }

    pub fn issues(&self, others: &[ScheduledJob], editing: Option<&str>) -> Vec<JobIssue> {
        let issue = |at, message: String| JobIssue { at, message };
        let mut out = Vec::new();
        let name = self.name.trim();
        if name.is_empty() {
            out.push(issue(JobField::Name, "Name the job.".into()));
        } else if !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            || !name.starts_with(|c: char| c.is_ascii_lowercase())
        {
            out.push(issue(
                JobField::Name,
                "Use lowercase letters, digits, and underscores, starting with a letter.".into(),
            ));
        } else if others
            .iter()
            .any(|j| j.name == name && Some(j.id.as_str()) != editing)
        {
            out.push(issue(
                JobField::Name,
                format!("A job named `{name}` exists. Choose another name."),
            ));
        }
        if let Err(e) = Schedule::parse(&self.cron) {
            out.push(issue(JobField::Schedule, e.to_string()));
        }
        if self.zone().is_none() {
            out.push(issue(
                JobField::Timezone,
                format!(
                    "`{}` is not a time zone Studio can schedule in.",
                    self.timezone
                ),
            ));
        }
        if self.source.trim().is_empty() {
            out.push(issue(
                JobField::Source,
                "Pick the source collection.".into(),
            ));
        }
        if self.sql.trim().is_empty() {
            out.push(issue(JobField::Sql, "Enter the SQL the job runs.".into()));
        }
        if self.owner.trim().is_empty() {
            out.push(issue(
                JobField::Owner,
                "Name the role the job runs as.".into(),
            ));
        }
        if self.retries_value().is_none() {
            out.push(issue(JobField::Retries, "Enter 0 to 10 retries.".into()));
        }
        out
    }

    /// The fields that differ from `job`, as `(label, was, now)`.
    pub fn changes_from(&self, job: &ScheduledJob) -> Vec<(&'static str, String, String)> {
        let was = JobForm::of(job);
        let mut out = Vec::new();
        let mut diff = |label: &'static str, a: String, b: String| {
            if a != b {
                out.push((label, a, b));
            }
        };
        diff("Name", was.name.clone(), self.name.clone());
        diff("Schedule", was.cron.clone(), self.cron.clone());
        diff("Time zone", was.timezone.clone(), self.timezone.clone());
        diff("Source", was.source.clone(), self.source.clone());
        diff("Owner", was.owner.clone(), self.owner.clone());
        diff(
            "Timeout",
            secs_label(was.timeout_secs),
            secs_label(self.timeout_secs),
        );
        diff(
            "Overlap policy",
            was.overlap.label().into(),
            self.overlap.label().into(),
        );
        diff("Retry attempts", was.retries.clone(), self.retries.clone());
        diff(
            "Backoff interval",
            secs_label(was.backoff_secs),
            secs_label(self.backoff_secs),
        );
        diff("SQL", line_summary(&was.sql), line_summary(&self.sql));
        out
    }
}

fn line_summary(sql: &str) -> String {
    let n = sql.lines().count();
    format!("{n} line{}", if n == 1 { "" } else { "s" })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::format::{date_minute, epoch};

    fn job() -> ScheduledJob {
        crate::data::mock::seeded_jobs().jobs.remove(0)
    }

    #[test]
    fn labels_pick_the_largest_whole_unit() {
        assert_eq!(secs_label(30), "30 s");
        assert_eq!(secs_label(300), "5 min");
        assert_eq!(secs_label(3600), "1 h");
        assert_eq!(secs_label(90), "90 s");
    }

    #[test]
    fn the_seeded_job_is_a_valid_form() {
        let j = job();
        assert!(JobForm::of(&j).issues(&[], None).is_empty());
    }

    #[test]
    fn a_bad_schedule_names_the_field() {
        let mut f = JobForm::of(&job());
        f.cron = "0 25 * * *".into();
        let issues = f.issues(&[], None);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].at, JobField::Schedule);
        assert_eq!(issues[0].message, "hour: `25` is outside 0-23");
        assert!(f.next_runs(0, 3).is_err());
    }

    #[test]
    fn next_runs_follow_the_schedule_and_zone() {
        let mut f = JobForm::of(&job());
        let now = epoch(2026, 6, 13, 13, 42, 30);
        let runs: Vec<_> = f
            .next_runs(now, 3)
            .expect("parses")
            .into_iter()
            .map(|t| date_minute(t, 0))
            .collect();
        assert_eq!(
            runs,
            ["2026-06-14 02:00", "2026-06-15 02:00", "2026-06-16 02:00"]
        );
        f.timezone = "Asia/Kuala_Lumpur".into();
        let first = f.next_runs(now, 1).expect("parses")[0];
        assert_eq!(date_minute(first, 0), "2026-06-13 18:00");
    }

    #[test]
    fn a_name_in_use_is_refused_unless_it_is_the_jobs_own() {
        let list = crate::data::mock::seeded_jobs().jobs;
        let mut f = JobForm::of(&list[0]);
        f.name = list[1].name.clone();
        assert_eq!(f.issues(&list, Some(&list[0].id))[0].at, JobField::Name);
        f.name = list[0].name.clone();
        assert!(f.issues(&list, Some(&list[0].id)).is_empty());
        assert_eq!(f.issues(&list, None)[0].at, JobField::Name);
    }

    #[test]
    fn a_blank_new_job_needs_a_name_and_sql() {
        let at: Vec<_> = JobForm::blank("analytics.sales", "rollup_service")
            .issues(&[], None)
            .iter()
            .map(|i| i.at)
            .collect();
        assert_eq!(at, [JobField::Name, JobField::Sql]);
    }

    #[test]
    fn an_edit_lists_only_what_changed() {
        let j = job();
        let mut f = JobForm::of(&j);
        assert!(f.changes_from(&j).is_empty());
        f.cron = "30 2 * * *".into();
        f.overlap = OverlapPolicy::Queue;
        let labels: Vec<_> = f.changes_from(&j).iter().map(|c| c.0).collect();
        assert_eq!(labels, ["Schedule", "Overlap policy"]);
    }

    #[test]
    fn retries_are_bounded() {
        let mut f = JobForm::of(&job());
        f.retries = "11".into();
        assert_eq!(f.issues(&[], None)[0].at, JobField::Retries);
        f.retries = "0".into();
        assert!(f.issues(&[], None).is_empty());
    }
}
