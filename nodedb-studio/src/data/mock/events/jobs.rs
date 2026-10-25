//! Scheduled job fixtures and the in-memory store the mock backend writes.

use std::collections::BTreeMap;

use crate::data::mock::events::topics::NOW;
use crate::models::format::date_second;
use crate::models::jobs::{
    JobForm, JobList, JobRun, LogLine, OverlapPolicy, RunState, ScheduledJob,
};
use crate::services::error::StudioError;

const ROLLUP_SQL: &str = "INSERT INTO sales_daily\nSELECT DATE_TRUNC('day', sold_at) AS day,\n    store_id, SUM(revenue) AS revenue_usd, COUNT(*) AS order_count\nFROM sales\nWHERE currency = 'USD'\nGROUP BY DATE_TRUNC('day', sold_at), store_id;";

pub fn seeded_jobs() -> JobList {
    JobList {
        observed_at: NOW,
        jobs: vec![
            ScheduledJob {
                id: "job_01".into(),
                name: "daily_sales_rollup".into(),
                enabled: true,
                cron: "0 2 * * *".into(),
                timezone: "UTC".into(),
                source: "analytics.sales".into(),
                sql: ROLLUP_SQL.into(),
                owner: "rollup_service".into(),
                timeout_secs: 300,
                overlap: OverlapPolicy::Skip,
                retries: 3,
                backoff_secs: 30,
                revision: 6,
            },
            ScheduledJob {
                id: "job_02".into(),
                name: "session_cleanup".into(),
                enabled: false,
                cron: "*/30 * * * *".into(),
                timezone: "UTC".into(),
                source: "analytics.sessions".into(),
                sql: "DELETE FROM sessions WHERE expires_at < now();".into(),
                owner: "maintenance_service".into(),
                timeout_secs: 60,
                overlap: OverlapPolicy::Skip,
                retries: 0,
                backoff_secs: 10,
                revision: 2,
            },
        ],
    }
}

fn run(job: &str, n: u32, started: &str, state: RunState, secs: u64, message: &str) -> JobRun {
    JobRun {
        id: format!("{job}_run{n}"),
        job: job.to_string(),
        started: started.to_string(),
        state,
        duration_secs: Some(secs),
        message: message.to_string(),
    }
}

fn seeded_runs() -> BTreeMap<String, Vec<JobRun>> {
    use RunState::{Failed, Success};
    let mut m = BTreeMap::new();
    m.insert(
        "job_01".to_string(),
        vec![
            run(
                "job_01",
                5,
                "2026-06-13 02:00:00",
                Success,
                82,
                "Completed 124 rows",
            ),
            run(
                "job_01",
                4,
                "2026-06-12 02:00:00",
                Failed,
                14,
                "permission denied",
            ),
            run(
                "job_01",
                3,
                "2026-06-11 02:00:00",
                Success,
                78,
                "Completed 12,304 rows",
            ),
            run(
                "job_01",
                2,
                "2026-06-10 02:00:00",
                Success,
                81,
                "Completed 12,276 rows",
            ),
            run(
                "job_01",
                1,
                "2026-06-09 02:00:00",
                Success,
                79,
                "Completed 12,361 rows",
            ),
        ],
    );
    m.insert(
        "job_02".to_string(),
        vec![
            run(
                "job_02",
                3,
                "2026-06-10 11:30:00",
                Success,
                4,
                "Deleted 312 rows",
            ),
            run(
                "job_02",
                2,
                "2026-06-10 11:00:00",
                Success,
                3,
                "Deleted 287 rows",
            ),
            run(
                "job_02",
                1,
                "2026-06-10 10:30:00",
                Success,
                4,
                "Deleted 301 rows",
            ),
        ],
    );
    m
}

/// The log of a finished run, from the run's own facts.
fn log_of(job: &ScheduledJob, r: &JobRun) -> Vec<LogLine> {
    let line = |at: &str, text: String| LogLine {
        at: at.to_string(),
        text,
    };
    let t0 = r.started.as_str();
    let secs = r.duration_secs.unwrap_or(0);
    let mut out = vec![
        line(t0, format!("Starting job {}", job.name)),
        line(t0, format!("Connecting to {}", job.source)),
    ];
    let t1 = clock_plus(t0, 1);
    out.push(line(&t1, "Executing query".to_string()));
    let end = clock_plus(t0, secs as i64);
    match r.state {
        RunState::Success => {
            out.push(line(&end, r.message.replace("Completed", "Returned")));
            out.push(line(
                &end,
                format!(
                    "Completed successfully in {}",
                    crate::models::format::duration(secs)
                ),
            ));
        }
        RunState::Failed => {
            out.push(line(
                &end,
                format!("ERROR {} for role {}", r.message, job.owner),
            ));
            out.push(line(
                &end,
                format!(
                    "Failed after {}. Retries: {}",
                    crate::models::format::duration(secs),
                    job.retries
                ),
            ));
        }
        RunState::Cancelled => out.push(line(&end, "Cancelled by admin".to_string())),
        RunState::Running => out.push(line(&t1, "Running".to_string())),
    }
    out
}

/// `2026-06-13 02:00:00` plus `secs`.
fn clock_plus(stamp: &str, secs: i64) -> String {
    let base = parse_stamp(stamp);
    date_second(base + secs, 0)
}

fn parse_stamp(stamp: &str) -> i64 {
    let n = |a: usize, b: usize| {
        stamp
            .get(a..b)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0)
    };
    crate::models::format::epoch(
        i64::from(n(0, 4)),
        n(5, 7),
        n(8, 10),
        n(11, 13),
        n(14, 16),
        n(17, 19),
    )
}

#[derive(Debug)]
pub struct JobStore {
    jobs: Vec<ScheduledJob>,
    runs: BTreeMap<String, Vec<JobRun>>,
    next_job: u32,
    next_run: u32,
    /// Reads a running run has seen. It finishes on the second.
    ticks: BTreeMap<String, u32>,
}

impl JobStore {
    pub fn seeded() -> Self {
        Self {
            jobs: seeded_jobs().jobs,
            runs: seeded_runs(),
            next_job: 3,
            next_run: 100,
            ticks: BTreeMap::new(),
        }
    }

    pub fn list(&self) -> JobList {
        JobList {
            observed_at: NOW,
            jobs: self.jobs.clone(),
        }
    }

    fn find(&self, id: &str) -> Result<&ScheduledJob, StudioError> {
        self.jobs
            .iter()
            .find(|j| j.id == id)
            .ok_or_else(|| StudioError::UnknownObject {
                kind: "scheduled job",
                id: id.to_string(),
            })
    }

    /// The runs of a job, newest first. A running run finishes on its
    /// second read.
    pub fn runs(&mut self, id: &str) -> Result<Vec<JobRun>, StudioError> {
        self.find(id)?;
        let runs = self.runs.entry(id.to_string()).or_default();
        for r in runs.iter_mut().filter(|r| r.state == RunState::Running) {
            let t = self.ticks.entry(r.id.clone()).or_insert(0);
            *t += 1;
            if *t >= 2 {
                r.state = RunState::Success;
                r.duration_secs = Some(82);
                r.message = "Completed 124 rows".to_string();
            }
        }
        Ok(runs.clone())
    }

    pub fn log(&self, run_id: &str) -> Result<Vec<LogLine>, StudioError> {
        for runs in self.runs.values() {
            if let Some(r) = runs.iter().find(|r| r.id == run_id) {
                let job = self.find(&r.job)?;
                return Ok(log_of(job, r));
            }
        }
        Err(StudioError::UnknownObject {
            kind: "job run",
            id: run_id.to_string(),
        })
    }

    fn reject(
        form: &JobForm,
        others: &[ScheduledJob],
        editing: Option<&str>,
    ) -> Result<(), StudioError> {
        match form.issues(others, editing).into_iter().next() {
            Some(i) => Err(StudioError::Rejected { reason: i.message }),
            None => Ok(()),
        }
    }

    fn apply(job: &mut ScheduledJob, form: &JobForm) {
        job.name = form.name.trim().to_string();
        job.cron = form.cron.trim().to_string();
        job.timezone = form.timezone.clone();
        job.source = form.source.trim().to_string();
        job.sql = form.sql.trim_end().to_string();
        job.owner = form.owner.trim().to_string();
        job.timeout_secs = form.timeout_secs;
        job.overlap = form.overlap;
        job.retries = form.retries_value().unwrap_or(job.retries);
        job.backoff_secs = form.backoff_secs;
    }

    pub fn create(&mut self, form: &JobForm) -> Result<ScheduledJob, StudioError> {
        Self::reject(form, &self.jobs, None)?;
        let mut job = ScheduledJob {
            id: format!("job_{:02}", self.next_job),
            name: String::new(),
            enabled: true,
            cron: String::new(),
            timezone: String::new(),
            source: String::new(),
            sql: String::new(),
            owner: String::new(),
            timeout_secs: 300,
            overlap: OverlapPolicy::Skip,
            retries: 0,
            backoff_secs: 30,
            revision: 1,
        };
        Self::apply(&mut job, form);
        self.next_job += 1;
        self.jobs.push(job.clone());
        Ok(job)
    }

    pub fn save(
        &mut self,
        id: &str,
        revision: u64,
        form: &JobForm,
    ) -> Result<ScheduledJob, StudioError> {
        let current = self.find(id)?;
        if current.revision != revision {
            return Err(StudioError::StaleRevision {
                object: format!("job `{}`", current.name),
                expected: revision,
                stored: current.revision,
            });
        }
        Self::reject(form, &self.jobs, Some(id))?;
        let job = self.jobs.iter_mut().find(|j| j.id == id).ok_or_else(|| {
            StudioError::UnknownObject {
                kind: "scheduled job",
                id: id.to_string(),
            }
        })?;
        Self::apply(job, form);
        job.revision += 1;
        Ok(job.clone())
    }

    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> Result<ScheduledJob, StudioError> {
        self.find(id)?;
        let job = self.jobs.iter_mut().find(|j| j.id == id).ok_or_else(|| {
            StudioError::UnknownObject {
                kind: "scheduled job",
                id: id.to_string(),
            }
        })?;
        job.enabled = enabled;
        job.revision += 1;
        Ok(job.clone())
    }

    /// Start a run now. Under the `Skip` policy a run in progress refuses it.
    pub fn run_now(&mut self, id: &str) -> Result<JobRun, StudioError> {
        let job = self.find(id)?.clone();
        let runs = self.runs.entry(id.to_string()).or_default();
        if job.overlap == OverlapPolicy::Skip && runs.iter().any(|r| r.state == RunState::Running) {
            return Err(StudioError::Rejected {
                reason: format!(
                    "{} is running and its overlap policy is Skip. Cancel the run or wait for it to end.",
                    job.name
                ),
            });
        }
        self.next_run += 1;
        let r = JobRun {
            id: format!("{id}_run{}", self.next_run),
            job: id.to_string(),
            started: date_second(NOW + i64::from(self.next_run - 100) * 60, 0),
            state: RunState::Running,
            duration_secs: None,
            message: "Started by admin".to_string(),
        };
        runs.insert(0, r.clone());
        Ok(r)
    }

    pub fn cancel(&mut self, run_id: &str) -> Result<JobRun, StudioError> {
        for runs in self.runs.values_mut() {
            if let Some(r) = runs.iter_mut().find(|r| r.id == run_id) {
                if r.state != RunState::Running {
                    return Err(StudioError::Rejected {
                        reason: format!(
                            "The run already ended: {}.",
                            r.state.label().to_lowercase()
                        ),
                    });
                }
                r.state = RunState::Cancelled;
                r.duration_secs = Some(3);
                r.message = "Cancelled by admin".to_string();
                return Ok(r.clone());
            }
        }
        Err(StudioError::UnknownObject {
            kind: "job run",
            id: run_id.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_seeded_jobs_match_the_design() {
        let l = seeded_jobs();
        let rows: Vec<_> = l
            .jobs
            .iter()
            .map(|j| (j.name.as_str(), j.enabled))
            .collect();
        assert_eq!(
            rows,
            [("daily_sales_rollup", true), ("session_cleanup", false)]
        );
        let j = &l.jobs[0];
        assert_eq!(
            (
                j.cron.as_str(),
                j.owner.as_str(),
                j.timeout_secs,
                j.retries,
                j.backoff_secs
            ),
            ("0 2 * * *", "rollup_service", 300, 3, 30)
        );
    }

    #[test]
    fn the_runs_match_the_design() {
        let mut s = JobStore::seeded();
        let runs = s.runs("job_01").expect("runs");
        let rows: Vec<_> = runs
            .iter()
            .map(|r| {
                (
                    r.started.as_str(),
                    r.state.label(),
                    r.duration_secs,
                    r.message.as_str(),
                )
            })
            .collect();
        assert_eq!(
            rows[0],
            (
                "2026-06-13 02:00:00",
                "Success",
                Some(82),
                "Completed 124 rows"
            )
        );
        assert_eq!(
            rows[1],
            (
                "2026-06-12 02:00:00",
                "Failed",
                Some(14),
                "permission denied"
            )
        );
        assert_eq!(rows.len(), 5);
    }

    #[test]
    fn the_design_log_has_five_lines() {
        let s = JobStore::seeded();
        let log = s.log("job_01_run5").expect("log");
        let text: Vec<_> = log.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(
            text,
            [
                "Starting job daily_sales_rollup",
                "Connecting to analytics.sales",
                "Executing query",
                "Returned 124 rows",
                "Completed successfully in 1m 22s",
            ]
        );
        assert_eq!(log[3].at, "2026-06-13 02:01:22");
        assert_eq!(log[2].at, "2026-06-13 02:00:01");
    }

    #[test]
    fn a_failed_run_log_names_the_error() {
        let s = JobStore::seeded();
        let log = s.log("job_01_run4").expect("log");
        assert!(
            log.iter()
                .any(|l| l.text.contains("ERROR permission denied"))
        );
    }

    #[test]
    fn run_ids_are_unique_across_jobs() {
        let mut s = JobStore::seeded();
        let mut ids = Vec::new();
        for j in ["job_01", "job_02"] {
            ids.extend(s.runs(j).expect("runs").into_iter().map(|r| r.id));
        }
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n);
    }

    #[test]
    fn run_now_starts_a_run_that_finishes_after_two_reads() {
        let mut s = JobStore::seeded();
        let r = s.run_now("job_01").expect("starts");
        assert_eq!(r.state, RunState::Running);
        assert_eq!(s.runs("job_01").expect("runs")[0].state, RunState::Running);
        assert_eq!(s.runs("job_01").expect("runs")[0].state, RunState::Success);
    }

    #[test]
    fn skip_refuses_a_second_run_and_cancel_ends_the_first() {
        let mut s = JobStore::seeded();
        let r = s.run_now("job_01").expect("starts");
        assert!(matches!(
            s.run_now("job_01"),
            Err(StudioError::Rejected { .. })
        ));
        let c = s.cancel(&r.id).expect("cancels");
        assert_eq!(c.state, RunState::Cancelled);
        assert!(s.cancel(&r.id).is_err());
        assert!(s.run_now("job_01").is_ok());
    }

    #[test]
    fn a_new_job_takes_the_next_id() {
        let mut s = JobStore::seeded();
        let mut f = JobForm::blank("analytics.sales", "rollup_service");
        f.name = "hourly_sales_check".into();
        f.sql = "SELECT 1;".into();
        let j = s.create(&f).expect("creates");
        assert_eq!((j.id.as_str(), j.enabled, j.revision), ("job_03", true, 1));
        assert!(s.create(&f).is_err());
    }

    #[test]
    fn a_stale_save_is_refused_and_a_save_bumps_the_revision() {
        let mut s = JobStore::seeded();
        let job = s.list().jobs.remove(0);
        let mut f = JobForm::of(&job);
        f.cron = "30 2 * * *".into();
        assert!(matches!(
            s.save(&job.id, job.revision - 1, &f),
            Err(StudioError::StaleRevision { .. })
        ));
        let saved = s.save(&job.id, job.revision, &f).expect("saves");
        assert_eq!(
            (saved.cron.as_str(), saved.revision),
            ("30 2 * * *", job.revision + 1)
        );
        f.cron = "nope".into();
        assert!(s.save(&job.id, saved.revision, &f).is_err());
    }

    #[test]
    fn disable_and_enable_name_the_job_and_bump_the_revision() {
        let mut s = JobStore::seeded();
        let off = s.set_enabled("job_01", false).expect("disables");
        assert!(!off.enabled);
        assert!(s.set_enabled("job_99", true).is_err());
    }
}
