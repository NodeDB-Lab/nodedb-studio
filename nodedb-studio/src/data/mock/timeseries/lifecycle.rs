//! The lifecycle of `sales`: retention, two rollups, the late-data policy.
//! Reviews and applies are bound to the revision they read.

use crate::models::format::{date_minute, thousands};
use crate::models::timeseries::Bucket;
use crate::models::ts_backfill::{BackfillRun, RunState};
use crate::models::ts_lifecycle::{
    BeyondHorizon, ChangeReview, LatePolicy, LateWrites, Lifecycle, LifecycleChange, Receipt,
    RefreshPolicy, Retention, ReviewItem, RollupDef, RollupFn, RollupOutput, RollupRun,
};

/// Why a review or an apply did not go through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleError {
    Stale { expected: u64, stored: u64 },
    Blocked(Vec<String>),
}

#[derive(Debug, Clone)]
pub struct LifecycleStore {
    pub lifecycle: Lifecycle,
    runs: Vec<RollupRun>,
}

fn out(func: RollupFn, column: Option<&str>, alias: &str) -> RollupOutput {
    RollupOutput {
        func,
        column: column.map(str::to_string),
        alias: alias.into(),
    }
}

fn outputs() -> Vec<RollupOutput> {
    vec![
        out(RollupFn::Sum, Some("revenue"), "revenue"),
        out(RollupFn::Count, None, "sale_count"),
    ]
}

impl LifecycleStore {
    pub fn seeded() -> Self {
        let dims = vec!["store_id".to_string(), "currency".to_string()];
        Self {
            lifecycle: Lifecycle {
                collection: "sales".into(),
                time_field: "sold_at".into(),
                partition: "1d".into(),
                raw_keep: Retention::Days(90),
                archive: None,
                rollups: vec![
                    RollupDef {
                        name: "sales_hourly".into(),
                        input: "sales".into(),
                        bucket: Bucket::Hour1,
                        outputs: outputs(),
                        group_by: dims.clone(),
                        refresh: RefreshPolicy::Interval("1m".into()),
                        keep: Retention::Days(365),
                    },
                    RollupDef {
                        name: "sales_daily".into(),
                        input: "sales_hourly".into(),
                        bucket: Bucket::Day1,
                        outputs: outputs(),
                        group_by: dims,
                        refresh: RefreshPolicy::OnSeal,
                        keep: Retention::CalendarYears(3),
                    },
                ],
                late: LatePolicy {
                    horizon_days: 7,
                    within: LateWrites::RecomputeAffected,
                    beyond: BeyondHorizon::Reject,
                },
                numeric_fields: vec!["revenue".into(), "quantity".into()],
                group_fields: vec!["store_id".into(), "product_id".into(), "currency".into()],
                revision: 1,
            },
            runs: Vec::new(),
        }
    }

    pub fn runs(&self, rollup: &str) -> Vec<RollupRun> {
        self.runs
            .iter()
            .filter(|r| r.rollup == rollup)
            .rev()
            .cloned()
            .collect()
    }

    /// Queue a refresh of one rollup.
    pub fn queue_refresh(
        &mut self,
        rollup: &str,
        operation: String,
        now: i64,
    ) -> Result<(), LifecycleError> {
        if self.lifecycle.rollup(rollup).is_none() {
            return Err(LifecycleError::Blocked(vec![format!(
                "There is no rollup named {rollup}."
            )]));
        }
        self.runs.push(RollupRun {
            operation,
            rollup: rollup.to_string(),
            requested: now,
            status: "Queued",
        });
        Ok(())
    }

    /// Look the changes over against the revision now held. Nothing changes.
    /// `raw_times` are the event times of the stored raw rows, and `jobs` the
    /// backfill history: an interrupted job still needs its raw rows.
    pub fn review(
        &self,
        changes: &[LifecycleChange],
        raw_times: &[i64],
        jobs: &[BackfillRun],
        now: i64,
    ) -> ChangeReview {
        let l = &self.lifecycle;
        let mut items = Vec::new();
        let mut blocked = Vec::new();
        for c in changes {
            match c {
                LifecycleChange::Retention(d) => {
                    if let Some(why) = d.issue() {
                        blocked.push(why);
                    }
                    items.push(retention_item(l, d, raw_times, jobs, now, &mut blocked));
                }
                LifecycleChange::CreateRollup(d) => {
                    blocked.extend(d.issues(l).into_iter().map(|i| i.message));
                    items.push(rollup_item(l, d));
                }
                LifecycleChange::LatePolicy(p) => {
                    if let Some(why) = p.issue() {
                        blocked.push(why.to_string());
                    }
                    items.push(late_item(l, p));
                }
            }
        }
        ChangeReview {
            revision: l.revision,
            items,
            blocked,
        }
    }

    /// Apply reviewed changes. A review of an older revision is refused: the
    /// catalog moved, so the impact it showed no longer holds.
    pub fn apply(
        &mut self,
        changes: &[LifecycleChange],
        reviewed: u64,
        raw_times: &[i64],
        jobs: &[BackfillRun],
        now: i64,
        operation: String,
    ) -> Result<Receipt, LifecycleError> {
        if reviewed != self.lifecycle.revision {
            return Err(LifecycleError::Stale {
                expected: reviewed,
                stored: self.lifecycle.revision,
            });
        }
        let review = self.review(changes, raw_times, jobs, now);
        if !review.blocked.is_empty() {
            return Err(LifecycleError::Blocked(review.blocked));
        }
        for c in changes {
            match c {
                LifecycleChange::Retention(d) => {
                    self.lifecycle.raw_keep = d.raw_keep;
                    for (name, keep) in &d.rollup_keep {
                        if let Some(r) = self.lifecycle.rollups.iter_mut().find(|r| r.name == *name)
                        {
                            r.keep = *keep;
                        }
                    }
                }
                LifecycleChange::CreateRollup(d) => self.lifecycle.rollups.push(d.def()),
                LifecycleChange::LatePolicy(p) => self.lifecycle.late = *p,
            }
        }
        self.lifecycle.revision += 1;
        Ok(Receipt {
            operation,
            applied: changes.len(),
            revision: self.lifecycle.revision,
        })
    }
}

fn retention_item(
    l: &Lifecycle,
    d: &crate::models::ts_lifecycle::RetentionDraft,
    raw_times: &[i64],
    jobs: &[BackfillRun],
    now: i64,
    blocked: &mut Vec<String>,
) -> ReviewItem {
    let mut sql =
        vec!["-- Proposed statements. Retention DDL is not confirmed on the server.".to_string()];
    let mut impact = Vec::new();
    let mut irreversible = false;
    if d.raw_keep != l.raw_keep {
        sql.push(format!(
            "ALTER COLLECTION {} SET retention = '{}';",
            l.collection,
            d.raw_keep.label()
        ));
        let cutoff = d.raw_keep.cutoff(now);
        let shorter = d.raw_keep.cutoff(now) > l.raw_keep.cutoff(now);
        if shorter {
            let expiring = raw_times.iter().filter(|t| **t < cutoff).count() as u64;
            irreversible = true;
            impact.push(format!(
                "Raw rows older than {} UTC expire: {} of {} stored rows today.",
                date_minute(cutoff, 0),
                thousands(expiring),
                thousands(raw_times.len() as u64)
            ));
            impact.push("Expiry cannot be undone. No archive is configured.".to_string());
            impact.push(format!(
                "{} keep their own retention and are not deleted.",
                join_names(l.rollups.iter().map(|r| r.name.as_str()))
            ));
            for run in jobs.iter().filter(|j| j.state == RunState::Interrupted) {
                if run.range.start < cutoff {
                    blocked.push(format!(
                        "{} is interrupted and still needs raw rows from {} UTC. Resume or cancel it first, or keep raw rows at least {} days.",
                        run.id,
                        date_minute(run.range.start, 0),
                        (now - run.range.start).div_euclid(86_400) + 1
                    ));
                }
            }
        } else {
            impact.push(
                "Rows already expired stay gone. Longer retention keeps newer rows only."
                    .to_string(),
            );
        }
        impact.push(format!(
            "A backfill older than {} can no longer recompute exact buckets from raw rows.",
            d.raw_keep.label()
        ));
    }
    for (name, keep) in &d.rollup_keep {
        let Some(r) = l.rollup(name) else { continue };
        if r.keep != *keep {
            sql.push(format!(
                "ALTER CONTINUOUS AGGREGATE {name} SET retention = '{}';",
                keep.label()
            ));
            if keep.cutoff(now) > r.keep.cutoff(now) {
                irreversible = true;
                impact.push(format!(
                    "{name} drops buckets older than {} UTC. Raw rows may already be gone, so they cannot be rebuilt.",
                    date_minute(keep.cutoff(now), 0)
                ));
            } else {
                impact.push(format!("{name} keeps buckets for {}.", keep.label()));
            }
        }
    }
    if sql.len() == 1 {
        impact.push("No retention differs from what is saved.".to_string());
    }
    ReviewItem {
        title: "Change retention".to_string(),
        sql: sql.join("\n"),
        impact,
        irreversible,
    }
}

fn rollup_item(l: &Lifecycle, d: &crate::models::ts_lifecycle::RollupDraft) -> ReviewItem {
    let def = d.def();
    let input_def = l.rollup(&d.input);
    let sql = def.sql(input_def.is_none(), input_def);
    let mut impact = vec![format!(
        "Reads {}. It adds no rows until its first refresh.",
        d.input
    )];
    impact.push(format!(
        "Retention {} is independent of {}'s {}.",
        d.keep.label(),
        d.input,
        input_def.map_or_else(|| l.raw_keep.label(), |r| r.keep.label())
    ));
    if !d.group_by.is_empty() {
        impact.push(format!("Keeps {} as dimensions.", d.group_by.join(", ")));
    }
    impact.push(format!(
        "No existing rollup depends on {}. Dropping it later previews its dependents.",
        if def.name.is_empty() {
            "it"
        } else {
            def.name.as_str()
        }
    ));
    ReviewItem {
        title: format!("Create rollup {}", def.name),
        sql,
        impact,
        irreversible: false,
    }
}

fn late_item(l: &Lifecycle, p: &LatePolicy) -> ReviewItem {
    ReviewItem {
        title: "Change late-data policy".to_string(),
        sql: format!(
            "-- Proposed statement. The server has no late-data DDL yet.\nALTER COLLECTION {} SET correction_horizon = '{}d',\n    late_writes = '{}',\n    beyond_horizon = '{}';",
            l.collection,
            p.horizon_days,
            p.within.label().to_lowercase().replace(' ', "_"),
            p.beyond.label().to_lowercase().replace(' ', "_"),
        ),
        impact: vec![
            format!(
                "Writes up to {} days late {}.",
                p.horizon_days,
                p.within.label().to_lowercase()
            ),
            format!(
                "Writes past the horizon: {}.",
                p.beyond.label().to_lowercase()
            ),
            "Only retained raw rows can correct a bucket. Expired rows stay uncorrectable."
                .to_string(),
        ],
        irreversible: false,
    }
}

fn join_names<'a>(names: impl Iterator<Item = &'a str>) -> String {
    let v: Vec<&str> = names.collect();
    match v.as_slice() {
        [] => "No rollup".to_string(),
        [one] => (*one).to_string(),
        [a, b] => format!("{a} and {b}"),
        [init @ .., last] => format!("{}, and {last}", init.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::format::epoch;
    use crate::models::timeseries::TimeSpan;
    use crate::models::ts_backfill::BackfillRun;
    use crate::models::ts_lifecycle::{RetentionDraft, RollupDraft};

    const NOW: i64 = epoch(2026, 6, 13, 13, 40, 0);

    fn times() -> Vec<i64> {
        // One row a day for the last 60 days.
        (0..60).map(|d| NOW - d * 86_400).collect()
    }

    fn interrupted() -> BackfillRun {
        BackfillRun {
            id: "backfill102".into(),
            target: "sales_daily".into(),
            state: RunState::Interrupted,
            range: TimeSpan::new(epoch(2026, 4, 11, 0, 0, 0), epoch(2026, 4, 18, 0, 0, 0)),
            started: 0,
            stopped: None,
            rows: 1,
            planned: 2,
            checkpoint: Some(1),
            idempotency_key: "k".into(),
        }
    }

    fn shorten(store: &LifecycleStore, days: u32) -> LifecycleChange {
        let mut d = RetentionDraft::from(&store.lifecycle);
        d.raw_keep = Retention::Days(days);
        LifecycleChange::Retention(d)
    }

    #[test]
    fn the_seed_matches_the_retention_table_the_design_shows() {
        let t = LifecycleStore::seeded().lifecycle.tiers();
        let row = |i: usize| {
            (
                t[i].name.clone(),
                t[i].input.clone(),
                t[i].bucket.map(Bucket::label),
                t[i].keep.label(),
                t[i].refresh.as_ref().map(RefreshPolicy::label),
            )
        };
        assert_eq!(row(0), ("sales".into(), None, None, "90d".into(), None));
        assert_eq!(
            row(1),
            (
                "sales_hourly".into(),
                Some("sales".into()),
                Some("1h"),
                "365d".into(),
                Some("Every 1m".into())
            )
        );
        assert_eq!(
            row(2),
            (
                "sales_daily".into(),
                Some("sales_hourly".into()),
                Some("1d"),
                "3y".into(),
                Some("On seal".into())
            )
        );
    }

    #[test]
    fn shortening_raw_retention_counts_the_rows_it_expires_and_is_irreversible() {
        let s = LifecycleStore::seeded();
        let r = s.review(&[shorten(&s, 30)], &times(), &[], NOW);
        assert!(r.blocked.is_empty());
        assert!(r.items[0].irreversible);
        assert!(
            r.items[0].impact[0].contains("29 of 60"),
            "{:?}",
            r.items[0].impact
        );
        assert!(r.items[0].sql.contains("SET retention = '30d'"));
    }

    #[test]
    fn lengthening_raw_retention_deletes_nothing() {
        let s = LifecycleStore::seeded();
        let r = s.review(&[shorten(&s, 180)], &times(), &[], NOW);
        assert!(!r.items[0].irreversible);
    }

    #[test]
    fn an_interrupted_backfill_blocks_a_retention_that_would_expire_its_input() {
        let s = LifecycleStore::seeded();
        let r = s.review(&[shorten(&s, 30)], &times(), &[interrupted()], NOW);
        assert_eq!(r.blocked.len(), 1);
        assert!(r.blocked[0].contains("backfill102"));
        assert!(
            r.blocked[0].contains("at least 64 days"),
            "{}",
            r.blocked[0]
        );
        // Keeping enough rows is allowed.
        let ok = s.review(&[shorten(&s, 64)], &times(), &[interrupted()], NOW);
        assert!(ok.blocked.is_empty());
    }

    #[test]
    fn apply_refuses_a_review_of_an_older_revision() {
        let mut s = LifecycleStore::seeded();
        let c = [shorten(&s, 60)];
        let err = s
            .apply(&c, 0, &times(), &[], NOW, "op-1".into())
            .expect_err("stale");
        assert_eq!(
            err,
            LifecycleError::Stale {
                expected: 0,
                stored: 1
            }
        );
        assert_eq!(s.lifecycle.raw_keep, Retention::Days(90));
    }

    #[test]
    fn apply_changes_the_catalog_and_returns_an_audit_receipt() {
        let mut s = LifecycleStore::seeded();
        let c = [shorten(&s, 60)];
        let r = s
            .apply(&c, 1, &times(), &[], NOW, "op-0105".into())
            .expect("applied");
        assert_eq!(
            (r.operation.as_str(), r.applied, r.revision),
            ("op-0105", 1, 2)
        );
        assert_eq!(s.lifecycle.raw_keep, Retention::Days(60));
        // The same review cannot be applied twice.
        assert!(s.apply(&c, 1, &times(), &[], NOW, "op-2".into()).is_err());
    }

    #[test]
    fn a_new_rollup_joins_the_tiers_once_applied() {
        let mut s = LifecycleStore::seeded();
        let mut d = RollupDraft::blank(&s.lifecycle);
        d.name = "sales_weekly".into();
        d.input = "sales_daily".into();
        d.bucket = Bucket::Day1;
        d.outputs = outputs();
        d.group_by = vec!["store_id".into(), "currency".into()];
        // A daily rollup of a daily rollup is not a coarser bucket.
        assert!(
            !s.review(&[LifecycleChange::CreateRollup(d.clone())], &[], &[], NOW)
                .blocked
                .is_empty()
        );
        let mut ok = RollupDraft::blank(&s.lifecycle);
        ok.name = "sales_6h".into();
        ok.input = "sales_hourly".into();
        ok.bucket = Bucket::Hour6;
        ok.outputs = outputs();
        ok.group_by = vec!["store_id".into(), "currency".into()];
        let c = [LifecycleChange::CreateRollup(ok)];
        let review = s.review(&c, &[], &[], NOW);
        assert!(review.blocked.is_empty(), "{:?}", review.blocked);
        assert!(
            review.items[0]
                .sql
                .contains("sum(sale_count) AS sale_count")
        );
        s.apply(&c, 1, &[], &[], NOW, "op-3".into())
            .expect("applied");
        assert!(s.lifecycle.rollup("sales_6h").is_some());
        assert_eq!(s.lifecycle.tiers().len(), 4);
    }

    #[test]
    fn a_blocked_review_cannot_be_applied() {
        let mut s = LifecycleStore::seeded();
        let d = RollupDraft::blank(&s.lifecycle);
        let c = [LifecycleChange::CreateRollup(d)];
        let err = s
            .apply(&c, 1, &[], &[], NOW, "op".into())
            .expect_err("blocked");
        assert!(matches!(err, LifecycleError::Blocked(_)));
        assert_eq!(s.lifecycle.rollups.len(), 2);
    }

    #[test]
    fn a_late_policy_change_is_reviewed_and_applied() {
        let mut s = LifecycleStore::seeded();
        let mut p = s.lifecycle.late;
        p.horizon_days = 14;
        let c = [LifecycleChange::LatePolicy(p)];
        let r = s.review(&c, &[], &[], NOW);
        assert!(r.items[0].sql.contains("correction_horizon = '14d'"));
        s.apply(&c, 1, &[], &[], NOW, "op".into()).expect("applied");
        assert_eq!(s.lifecycle.late.horizon_days, 14);
    }

    #[test]
    fn a_refresh_is_queued_without_a_progress_claim() {
        let mut s = LifecycleStore::seeded();
        s.queue_refresh("sales_hourly", "op-9".into(), NOW)
            .expect("queued");
        let runs = s.runs("sales_hourly");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, "Queued");
        assert!(s.runs("sales_daily").is_empty());
        assert!(s.queue_refresh("nope", "op".into(), NOW).is_err());
    }
}
