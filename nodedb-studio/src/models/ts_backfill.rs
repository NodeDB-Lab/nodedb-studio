//! Backfill: recompute a rollup's buckets from the retained source rows.
//!
//! A preview is immutable. It names the half-open range it expanded to, the
//! buckets it would replace, and the rows it would read. A run names the
//! preview revision and an idempotency key, so a retry or a resume cannot
//! count a row twice.

use crate::models::format::{date, epoch};
use crate::models::timeseries::{Bucket, TimeSpan, is_identifier};
use crate::models::ts_lifecycle::{LateWrites, RollupFn, RollupOutput};

/// What a run does with a bucket the target already holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictPolicy {
    ReplaceAffected,
    KeepExisting,
    Fail,
}

impl ConflictPolicy {
    pub const ALL: [ConflictPolicy; 3] = [
        ConflictPolicy::ReplaceAffected,
        ConflictPolicy::KeepExisting,
        ConflictPolicy::Fail,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ConflictPolicy::ReplaceAffected => "Replace affected buckets",
            ConflictPolicy::KeepExisting => "Keep existing buckets",
            ConflictPolicy::Fail => "Fail on conflict",
        }
    }
}

/// How a run writes the buckets it computes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteMode {
    ReplaceAffected,
    FillMissing,
}

impl WriteMode {
    pub const ALL: [WriteMode; 2] = [WriteMode::ReplaceAffected, WriteMode::FillMissing];

    pub fn label(self) -> &'static str {
        match self {
            WriteMode::ReplaceAffected => "Replace affected buckets",
            WriteMode::FillMissing => "Fill missing buckets only",
        }
    }
}

/// What the Run backfill panel will do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RunMode {
    #[default]
    DryRun,
    Run,
}

/// Everything a backfill job is defined by.
#[derive(Debug, Clone, PartialEq)]
pub struct BackfillSpec {
    /// The rollup to recompute.
    pub target: String,
    /// The target's bucket, which the range expands to.
    pub bucket: Bucket,
    /// The collection the rows come from, with its database.
    pub source: String,
    pub time_field: String,
    /// The range as asked, half-open, UTC.
    pub range: TimeSpan,
    pub aggregates: Vec<RollupOutput>,
    pub currency: Option<String>,
    pub late: LateWrites,
    pub job_name: String,
    pub description: String,
    pub mode: WriteMode,
    pub conflict: ConflictPolicy,
    /// The key that identifies a bucket in the target: `day + store_id`.
    pub target_key: String,
    pub checkpoint: bool,
}

impl BackfillSpec {
    /// The job the Lifecycle tab offers for `sales_daily` before anything
    /// is changed: the first week of June, rebuilt from the retained rows.
    pub fn standard() -> Self {
        Self {
            target: "sales_daily".to_string(),
            bucket: Bucket::Day1,
            source: "analytics.sales".to_string(),
            time_field: "sold_at".to_string(),
            range: TimeSpan::new(epoch(2026, 6, 1, 0, 0, 0), epoch(2026, 6, 8, 0, 0, 0)),
            aggregates: vec![
                RollupOutput {
                    func: RollupFn::Sum,
                    column: Some("revenue".to_string()),
                    alias: "revenue".to_string(),
                },
                RollupOutput {
                    func: RollupFn::Count,
                    column: None,
                    alias: "sale_count".to_string(),
                },
            ],
            currency: Some("USD".to_string()),
            late: LateWrites::RecomputeAffected,
            job_name: "backfill104".to_string(),
            description: "Backfill June 1–7, 2026 for sales_daily".to_string(),
            mode: WriteMode::ReplaceAffected,
            conflict: ConflictPolicy::ReplaceAffected,
            target_key: "day + store_id".to_string(),
            checkpoint: true,
        }
    }

    /// The range widened to whole target buckets: the start rounds down and
    /// the end rounds up, so every affected bucket is whole.
    pub fn expanded(&self) -> TimeSpan {
        let size = self.bucket.seconds();
        TimeSpan::new(
            self.range.start.div_euclid(size) * size,
            (self.range.end + size - 1).div_euclid(size) * size,
        )
    }

    pub fn issues(&self) -> Vec<String> {
        let mut out = Vec::new();
        if !is_identifier(&self.job_name) {
            out.push("Name the job with letters, digits, and underscores.".to_string());
        }
        if self.range.end <= self.range.start {
            out.push("The range must end after it starts.".to_string());
        }
        if self.aggregates.is_empty() {
            out.push("Choose at least one aggregate.".to_string());
        }
        if self.target_key.trim().is_empty() {
            out.push("Name the target key.".to_string());
        }
        out
    }

    /// A hash of everything that decides what a run computes. The job name
    /// and description do not: renaming a job does not change its preview.
    fn fingerprint(&self) -> u64 {
        let text = format!(
            "{}|{}|{}|{}|{}|{}|{:?}|{:?}|{:?}|{:?}|{}|{}",
            self.target,
            self.source,
            self.time_field,
            self.range.start,
            self.range.end,
            self.aggregates
                .iter()
                .map(|a| a.logical())
                .collect::<Vec<_>>()
                .join(","),
            self.currency,
            self.late,
            self.mode,
            self.conflict,
            self.target_key,
            self.checkpoint,
        );
        fnv1a(&text)
    }

    /// The preview revision for this spec. The same spec always previews to
    /// the same revision: a preview does not change under the user.
    pub fn preview_revision(&self) -> String {
        format!("prev-{:08x}", self.fingerprint() as u32)
    }

    /// The key a run is submitted under. A second submission with it is the
    /// same run, never a second one. It names the job as well as the data.
    pub fn idempotency_key(&self, preview_revision: &str) -> String {
        let h = fnv1a(&format!(
            "{}|{}|{preview_revision}",
            self.fingerprint(),
            self.job_name
        ));
        format!("bf-{h:016x}")
    }
}

/// FNV-1a. Stable across runs, which `DefaultHasher` does not promise.
pub fn fnv1a(text: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// One affected bucket: what the target holds and what the source gives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BucketDelta {
    pub start: i64,
    /// Rows the target bucket counts now. `None` when it holds no bucket.
    pub existing: Option<u64>,
    /// Rows the recompute would count. `None` when the source cannot say.
    pub proposed: Option<u64>,
    /// Source rows read for this bucket, filtered or not.
    pub scanned: u64,
    /// The source still holds this bucket's rows.
    pub retained: bool,
}

impl BucketDelta {
    pub fn change(&self) -> Option<i64> {
        Some(self.proposed? as i64 - self.existing.unwrap_or(0) as i64)
    }

    pub fn day(&self) -> String {
        date(self.start, 0)
    }
}

/// The result of a dry run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackfillPreview {
    pub revision: String,
    pub requested: TimeSpan,
    pub range: TimeSpan,
    pub buckets: Vec<BucketDelta>,
    /// Source rows the run would read.
    pub scanned_rows: u64,
    /// `None` is not reported: the server gives no estimate.
    pub cost: Option<String>,
    /// Rollups cut from the target, recomputed after it.
    pub dependents: Vec<String>,
    pub snapshot_at: i64,
}

impl BackfillPreview {
    /// Buckets whose source rows are past retention.
    pub fn unavailable(&self) -> Vec<&BucketDelta> {
        self.buckets.iter().filter(|b| !b.retained).collect()
    }

    pub fn expanded(&self) -> bool {
        self.requested != self.range
    }

    pub fn changed(&self) -> usize {
        self.buckets
            .iter()
            .filter(|b| b.change().is_some_and(|c| c != 0))
            .count()
    }
}

/// Where a job stands. The mock finishes a job at once. A server reports
/// the other states while a job runs or after it is stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "services::nodedb::timeseries builds it from a queued server job; that service is a stub today"
        )
    )]
    Queued,
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "services::nodedb::timeseries builds it from a running server job; that service is a stub today"
        )
    )]
    Running,
    Completed,
    Interrupted,
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "services::nodedb::timeseries builds it from a cancelled server job; that service is a stub today"
        )
    )]
    Cancelled,
}

impl RunState {
    pub fn label(self) -> &'static str {
        match self {
            RunState::Queued => "Queued",
            RunState::Running => "Running",
            RunState::Completed => "Completed",
            RunState::Interrupted => "Interrupted",
            RunState::Cancelled => "Cancelled",
        }
    }

    /// The `status-dot` tone.
    pub fn tone(self) -> &'static str {
        match self {
            RunState::Completed => "ok",
            RunState::Interrupted => "err",
            RunState::Queued | RunState::Running => "info",
            RunState::Cancelled => "idle",
        }
    }
}

/// One backfill job in the history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackfillRun {
    pub id: String,
    pub target: String,
    pub state: RunState,
    pub range: TimeSpan,
    pub started: i64,
    pub stopped: Option<i64>,
    /// Rows contributed so far.
    pub rows: u64,
    /// Rows the whole job reads.
    pub planned: u64,
    /// The instant up to which buckets are published. A resume starts here.
    pub checkpoint: Option<i64>,
    pub idempotency_key: String,
}

impl BackfillRun {
    pub fn resumable(&self) -> bool {
        self.state == RunState::Interrupted && self.checkpoint.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::format::epoch;
    use crate::models::ts_lifecycle::RollupFn;

    #[test]
    fn server_reported_run_states_have_labels_and_tones() {
        assert_eq!(
            (RunState::Queued.label(), RunState::Queued.tone()),
            ("Queued", "info")
        );
        assert_eq!(
            (RunState::Running.label(), RunState::Running.tone()),
            ("Running", "info")
        );
        assert_eq!(
            (RunState::Cancelled.label(), RunState::Cancelled.tone()),
            ("Cancelled", "idle")
        );
    }

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

    #[test]
    fn a_range_that_is_already_whole_buckets_does_not_expand() {
        assert_eq!(spec().expanded(), spec().range);
    }

    #[test]
    fn a_range_expands_to_whole_buckets_on_both_sides() {
        let mut s = spec();
        s.range = TimeSpan::new(epoch(2026, 6, 1, 6, 0, 0), epoch(2026, 6, 7, 18, 0, 0));
        let e = s.expanded();
        assert_eq!(e.start, epoch(2026, 6, 1, 0, 0, 0));
        assert_eq!(e.end, epoch(2026, 6, 8, 0, 0, 0));
    }

    #[test]
    fn the_same_spec_previews_to_the_same_revision() {
        assert_eq!(spec().preview_revision(), spec().preview_revision());
        let mut other = spec();
        other.range.end += 86_400;
        assert_ne!(spec().preview_revision(), other.preview_revision());
    }

    #[test]
    fn renaming_a_job_keeps_its_preview_and_changes_its_key() {
        let a = spec();
        let mut b = spec();
        b.job_name = "backfill105".into();
        b.description = "other".into();
        assert_eq!(a.preview_revision(), b.preview_revision());
        let rev = a.preview_revision();
        assert_ne!(a.idempotency_key(&rev), b.idempotency_key(&rev));
    }

    #[test]
    fn anything_that_changes_the_result_changes_the_preview() {
        let base = spec().preview_revision();
        let mut s = spec();
        s.currency = None;
        assert_ne!(base, s.preview_revision());
        let mut s = spec();
        s.mode = WriteMode::FillMissing;
        assert_ne!(base, s.preview_revision());
        let mut s = spec();
        s.late = LateWrites::Reject;
        assert_ne!(base, s.preview_revision());
    }

    #[test]
    fn the_idempotency_key_is_stable_and_tied_to_the_preview() {
        let s = spec();
        let rev = s.preview_revision();
        assert_eq!(s.idempotency_key(&rev), s.idempotency_key(&rev));
        assert_ne!(s.idempotency_key(&rev), s.idempotency_key("prev-00000000"));
        assert!(s.idempotency_key(&rev).starts_with("bf-"));
    }

    #[test]
    fn a_spec_without_a_name_or_aggregate_has_issues() {
        let mut s = spec();
        assert!(s.issues().is_empty());
        s.job_name = "a b".into();
        s.aggregates.clear();
        assert_eq!(s.issues().len(), 2);
    }

    #[test]
    fn a_bucket_with_no_proposal_has_no_change() {
        let b = BucketDelta {
            start: 0,
            existing: Some(5),
            proposed: None,
            scanned: 0,
            retained: false,
        };
        assert_eq!(b.change(), None);
        let b = BucketDelta {
            proposed: Some(8),
            retained: true,
            ..b
        };
        assert_eq!(b.change(), Some(3));
    }

    #[test]
    fn only_an_interrupted_run_with_a_checkpoint_resumes() {
        let run = BackfillRun {
            id: "backfill102".into(),
            target: "sales_daily".into(),
            state: RunState::Interrupted,
            range: spec().range,
            started: 0,
            stopped: Some(1),
            rows: 10,
            planned: 20,
            checkpoint: Some(5),
            idempotency_key: "bf-1".into(),
        };
        assert!(run.resumable());
        assert!(
            !BackfillRun {
                state: RunState::Completed,
                ..run.clone()
            }
            .resumable()
        );
        assert!(
            !BackfillRun {
                checkpoint: None,
                ..run
            }
            .resumable()
        );
    }
}
