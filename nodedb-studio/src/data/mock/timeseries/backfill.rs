//! Backfill dry runs and job history for the rollups of `sales`.

use crate::data::mock::explorer::build::unit;
use crate::models::format::epoch;
use crate::models::timeseries::TimeSpan;
use crate::models::ts_backfill::{
    BackfillPreview, BackfillRun, BackfillSpec, BucketDelta, RunState,
};
use crate::models::ts_lifecycle::Lifecycle;

/// Why a backfill did not start or resume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackfillError {
    Invalid(String),
    UnknownRun(String),
    NameInUse(String),
    /// The preview the run names is not the preview of this spec.
    StalePreview {
        given: String,
        current: String,
    },
}

/// 1–7 June 2026 as the target holds them and the source would give them:
/// existing buckets, proposed buckets, and source rows read per day.
const DAYS: [(u32, u64, u64, u64); 7] = [
    (1, 1_420, 1_438, 1_600),
    (2, 1_385, 1_391, 1_540),
    (3, 1_672, 1_701, 1_880),
    (4, 1_553, 1_559, 1_720),
    (5, 1_698, 1_725, 1_920),
    (6, 1_410, 1_428, 1_610),
    (7, 1_472, 1_498, 1_770),
];

#[derive(Debug, Clone)]
pub struct RunStore {
    runs: Vec<BackfillRun>,
}

fn run(
    id: &str,
    target: &str,
    state: RunState,
    range: (i64, i64),
    when: (i64, i64),
    rows: (u64, u64),
    checkpoint: i64,
) -> BackfillRun {
    BackfillRun {
        id: id.into(),
        target: target.into(),
        state,
        range: TimeSpan::new(range.0, range.1),
        started: when.0,
        stopped: Some(when.1),
        rows: rows.0,
        planned: rows.1,
        checkpoint: Some(checkpoint),
        idempotency_key: format!("bf-seed-{id}"),
    }
}

/// The preview of `spec` against the rollup lifecycle, as of `now`.
pub fn preview(spec: &BackfillSpec, lifecycle: &Lifecycle, now: i64) -> BackfillPreview {
    let range = spec.expanded();
    let size = spec.bucket.seconds();
    let raw_floor = lifecycle.raw_keep.cutoff(now).div_euclid(size) * size;
    let june_first = epoch(2026, 6, 1, 0, 0, 0);
    let buckets: Vec<BucketDelta> = range
        .bucket_starts(spec.bucket)
        .into_iter()
        .filter(|s| *s < range.end)
        .map(|start| {
            let fixed = (size == 86_400)
                .then(|| DAYS.iter().find(|d| epoch(2026, 6, d.0, 0, 0, 0) == start))
                .flatten();
            // A day outside the authored week holds a steady count, which a
            // recompute leaves as it is. Finer buckets share the day's rows.
            let steady = {
                let day = (start - june_first).div_euclid(86_400) as usize;
                let per_day = 1_300.0 + unit(day.wrapping_add(7), 11) * 400.0;
                ((per_day * size as f64 / 86_400.0).round() as u64).max(1)
            };
            let retained = start >= raw_floor;
            let (existing, proposed, scanned) = match fixed {
                Some((_, e, p, s)) => (*e, *p, *s),
                None => (steady, steady, steady + steady / 9),
            };
            BucketDelta {
                start,
                existing: Some(existing),
                proposed: retained.then_some(proposed),
                scanned: if retained { scanned } else { 0 },
                retained,
            }
        })
        .collect();
    BackfillPreview {
        revision: spec.preview_revision(),
        requested: spec.range,
        range,
        scanned_rows: buckets.iter().map(|b| b.scanned).sum(),
        cost: None,
        dependents: lifecycle.dependents(&spec.target),
        snapshot_at: now,
        buckets,
    }
}

impl RunStore {
    pub fn seeded() -> Self {
        let d = |y, m, day, h, mi| epoch(y, m, day, h, mi, 0);
        Self {
            runs: vec![
                run(
                    "backfill-014",
                    "sales_hourly",
                    RunState::Completed,
                    (d(2026, 6, 10, 0, 0), d(2026, 6, 11, 0, 0)),
                    (d(2026, 6, 11, 9, 2), d(2026, 6, 11, 9, 9)),
                    (4_380, 4_380),
                    d(2026, 6, 11, 9, 8),
                ),
                run(
                    "backfill103",
                    "sales_daily",
                    RunState::Completed,
                    (d(2026, 5, 18, 0, 0), d(2026, 5, 25, 0, 0)),
                    (d(2026, 5, 25, 10, 12), d(2026, 5, 25, 10, 28)),
                    (15_420, 15_420),
                    d(2026, 5, 25, 10, 20),
                ),
                run(
                    "backfill102",
                    "sales_daily",
                    RunState::Interrupted,
                    (d(2026, 4, 11, 0, 0), d(2026, 4, 18, 0, 0)),
                    (d(2026, 4, 18, 8, 41), d(2026, 4, 18, 9, 5)),
                    (8_210, 11_840),
                    d(2026, 4, 18, 8, 58),
                ),
                run(
                    "backfill101",
                    "sales_daily",
                    RunState::Completed,
                    (d(2026, 2, 24, 0, 0), d(2026, 3, 3, 0, 0)),
                    (d(2026, 3, 3, 14, 17), d(2026, 3, 3, 14, 46)),
                    (22_006, 22_006),
                    d(2026, 3, 3, 14, 40),
                ),
            ],
        }
    }

    /// Newest first. `None` lists every target.
    pub fn runs(&self, target: Option<&str>) -> Vec<BackfillRun> {
        let mut v: Vec<BackfillRun> = self
            .runs
            .iter()
            .filter(|r| target.is_none_or(|t| r.target == t))
            .cloned()
            .collect();
        v.sort_by_key(|r| std::cmp::Reverse(r.started));
        v
    }

    pub fn all(&self) -> &[BackfillRun] {
        &self.runs
    }

    /// Start a run under the preview it was reviewed on. The run completes
    /// at once in the mock. A second submission with the same key is the
    /// same run: nothing is counted twice.
    pub fn start(
        &mut self,
        spec: &BackfillSpec,
        preview_revision: &str,
        key: &str,
        lifecycle: &Lifecycle,
        now: i64,
    ) -> Result<BackfillRun, BackfillError> {
        if let Some(existing) = self.runs.iter().find(|r| r.idempotency_key == key) {
            return Ok(existing.clone());
        }
        let issues = spec.issues();
        if let Some(first) = issues.first() {
            return Err(BackfillError::Invalid(first.clone()));
        }
        let current = spec.preview_revision();
        if current != preview_revision || spec.idempotency_key(&current) != key {
            return Err(BackfillError::StalePreview {
                given: preview_revision.to_string(),
                current,
            });
        }
        if self.runs.iter().any(|r| r.id == spec.job_name) {
            return Err(BackfillError::NameInUse(spec.job_name.clone()));
        }
        let planned = preview(spec, lifecycle, now).scanned_rows;
        let run = BackfillRun {
            id: spec.job_name.clone(),
            target: spec.target.clone(),
            state: RunState::Completed,
            range: spec.expanded(),
            started: now,
            stopped: Some(now + 94),
            rows: planned,
            planned,
            checkpoint: Some(spec.expanded().end),
            idempotency_key: key.to_string(),
        };
        self.runs.push(run.clone());
        Ok(run)
    }

    /// Continue an interrupted run from its checkpoint. Rows up to the
    /// checkpoint are not read again.
    pub fn resume(&mut self, id: &str, now: i64) -> Result<BackfillRun, BackfillError> {
        let r = self
            .runs
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or_else(|| BackfillError::UnknownRun(id.to_string()))?;
        if !r.resumable() {
            return Err(BackfillError::Invalid(format!(
                "{id} is {} and has nothing to resume.",
                r.state.label().to_lowercase()
            )));
        }
        r.state = RunState::Completed;
        r.rows = r.planned;
        r.stopped = Some(now);
        r.checkpoint = Some(r.range.end);
        Ok(r.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::mock::timeseries::lifecycle::LifecycleStore;
    use crate::models::timeseries::Bucket;
    use crate::models::ts_backfill::{ConflictPolicy, WriteMode};
    use crate::models::ts_lifecycle::{LateWrites, RollupFn, RollupOutput};

    const NOW: i64 = epoch(2026, 6, 13, 13, 40, 0);

    fn spec() -> BackfillSpec {
        BackfillSpec {
            target: "sales_daily".into(),
            bucket: Bucket::Day1,
            source: "analytics.sales".into(),
            time_field: "sold_at".into(),
            range: TimeSpan::new(epoch(2026, 6, 1, 0, 0, 0), epoch(2026, 6, 8, 0, 0, 0)),
            aggregates: vec![RollupOutput {
                func: RollupFn::Sum,
                column: Some("revenue".into()),
                alias: "revenue".into(),
            }],
            currency: Some("USD".into()),
            late: LateWrites::RecomputeAffected,
            job_name: "backfill104".into(),
            description: "Backfill June 1-7, 2026 for sales_daily".into(),
            mode: WriteMode::ReplaceAffected,
            conflict: ConflictPolicy::ReplaceAffected,
            target_key: "day + store_id".into(),
            checkpoint: true,
        }
    }

    fn lifecycle() -> Lifecycle {
        LifecycleStore::seeded().lifecycle
    }

    #[test]
    fn the_june_week_previews_the_counts_the_design_shows() {
        let p = preview(&spec(), &lifecycle(), NOW);
        let got: Vec<(String, u64, u64)> = p
            .buckets
            .iter()
            .map(|b| (b.day(), b.existing.unwrap_or(0), b.proposed.unwrap_or(0)))
            .collect();
        assert_eq!(got.len(), 7);
        assert_eq!(got[0], ("2026-06-01".into(), 1_420, 1_438));
        assert_eq!(got[2], ("2026-06-03".into(), 1_672, 1_701));
        assert_eq!(got[6], ("2026-06-07".into(), 1_472, 1_498));
        assert_eq!(p.scanned_rows, 12_040);
        assert_eq!(p.cost, None, "cost is not reported, never invented");
    }

    #[test]
    fn the_changes_are_the_difference_of_the_counts() {
        let p = preview(&spec(), &lifecycle(), NOW);
        let changes: Vec<i64> = p.buckets.iter().filter_map(|b| b.change()).collect();
        assert_eq!(changes, [18, 6, 29, 6, 27, 18, 26]);
        assert_eq!(p.changed(), 7);
    }

    #[test]
    fn a_range_inside_a_day_expands_to_the_whole_day_and_says_so() {
        let mut s = spec();
        s.range = TimeSpan::new(epoch(2026, 6, 2, 6, 0, 0), epoch(2026, 6, 3, 6, 0, 0));
        let p = preview(&s, &lifecycle(), NOW);
        assert!(p.expanded());
        assert_eq!(p.buckets.len(), 2);
        assert_eq!(p.range.end, epoch(2026, 6, 4, 0, 0, 0));
    }

    #[test]
    fn buckets_past_raw_retention_are_unavailable_not_invented() {
        let mut s = spec();
        s.range = TimeSpan::new(epoch(2026, 3, 13, 0, 0, 0), epoch(2026, 3, 17, 0, 0, 0));
        let p = preview(&s, &lifecycle(), NOW);
        let gone = p.unavailable();
        assert!(!gone.is_empty() && gone.len() < p.buckets.len());
        assert!(gone.iter().all(|b| b.proposed.is_none() && b.scanned == 0));
        assert!(gone.iter().all(|b| b.change().is_none()));
    }

    #[test]
    fn the_hourly_target_lists_hour_buckets_and_its_dependents() {
        let mut s = spec();
        s.target = "sales_hourly".into();
        s.bucket = Bucket::Hour1;
        s.range = TimeSpan::new(epoch(2026, 6, 12, 0, 0, 0), epoch(2026, 6, 13, 0, 0, 0));
        let p = preview(&s, &lifecycle(), NOW);
        assert_eq!(p.buckets.len(), 24);
        assert_eq!(p.dependents, ["sales_daily"]);
    }

    #[test]
    fn a_preview_is_the_same_every_time() {
        assert_eq!(
            preview(&spec(), &lifecycle(), NOW),
            preview(&spec(), &lifecycle(), NOW)
        );
    }

    #[test]
    fn a_run_submitted_twice_under_one_key_is_one_run() {
        let mut store = RunStore::seeded();
        let s = spec();
        let rev = s.preview_revision();
        let key = s.idempotency_key(&rev);
        let first = store.start(&s, &rev, &key, &lifecycle(), NOW).expect("run");
        let second = store
            .start(&s, &rev, &key, &lifecycle(), NOW + 60)
            .expect("same run");
        assert_eq!(first, second);
        assert_eq!(store.runs(None).len(), 5);
        assert_eq!(first.rows, 12_040);
        assert_eq!(first.state, RunState::Completed);
    }

    #[test]
    fn a_run_on_a_changed_spec_needs_a_new_preview() {
        let mut store = RunStore::seeded();
        let s = spec();
        let rev = s.preview_revision();
        let key = s.idempotency_key(&rev);
        let mut changed = s.clone();
        changed.range.end += 86_400;
        let err = store
            .start(&changed, &rev, &key, &lifecycle(), NOW)
            .expect_err("stale");
        assert!(matches!(err, BackfillError::StalePreview { .. }));
        assert_eq!(store.runs(None).len(), 4);
    }

    #[test]
    fn a_job_name_in_use_is_refused_under_a_new_key() {
        let mut store = RunStore::seeded();
        let mut s = spec();
        s.job_name = "backfill103".into();
        let rev = s.preview_revision();
        let key = s.idempotency_key(&rev);
        assert!(matches!(
            store.start(&s, &rev, &key, &lifecycle(), NOW),
            Err(BackfillError::NameInUse(_))
        ));
    }

    #[test]
    fn resume_finishes_an_interrupted_run_without_reading_rows_twice() {
        let mut store = RunStore::seeded();
        let r = store.resume("backfill102", NOW).expect("resumed");
        assert_eq!(r.state, RunState::Completed);
        assert_eq!(r.rows, 11_840);
        assert!(r.rows <= r.planned);
        assert!(
            store.resume("backfill102", NOW).is_err(),
            "nothing left to resume"
        );
        assert!(store.resume("backfill103", NOW).is_err());
        assert!(matches!(
            store.resume("nope", NOW),
            Err(BackfillError::UnknownRun(_))
        ));
    }

    #[test]
    fn history_lists_newest_first_and_filters_by_target() {
        let store = RunStore::seeded();
        let all: Vec<String> = store.runs(None).into_iter().map(|r| r.id).collect();
        assert_eq!(
            all,
            ["backfill-014", "backfill103", "backfill102", "backfill101"]
        );
        let daily: Vec<String> = store
            .runs(Some("sales_daily"))
            .into_iter()
            .map(|r| r.id)
            .collect();
        assert_eq!(daily, ["backfill103", "backfill102", "backfill101"]);
    }
}
