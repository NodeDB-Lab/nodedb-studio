//! Lifecycle of a timeseries collection: raw retention, rollups, and the
//! late-data policy.
//!
//! A saved definition is not a runtime state. Nothing here says a rollup is
//! fresh, complete, or backfilled: those facts come from measured telemetry
//! and read `None` until the server reports them.

use crate::models::format::{civil, epoch};
use crate::models::timeseries::Bucket;
pub use crate::models::ts_rollup::{RefreshPolicy, RollupDef, RollupFn, RollupOutput};
pub use crate::models::ts_rollup_draft::{DraftField, RollupDraft};

/// A retention length. Years are calendar years, not 365 days, because the
/// two differ across a leap year.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retention {
    Days(u32),
    CalendarYears(u32),
}

pub const MAX_RETENTION_DAYS: u32 = 36_500;

impl Retention {
    /// `90d` or `3y`.
    pub fn label(self) -> String {
        match self {
            Retention::Days(n) => format!("{n}d"),
            Retention::CalendarYears(n) => format!("{n}y"),
        }
    }

    pub fn amount(self) -> u32 {
        match self {
            Retention::Days(n) | Retention::CalendarYears(n) => n,
        }
    }

    /// Rows older than this instant are past retention at `now`.
    pub fn cutoff(self, now: i64) -> i64 {
        match self {
            Retention::Days(n) => now - i64::from(n) * 86_400,
            Retention::CalendarYears(n) => {
                let c = civil(now, 0);
                // 29 February in a non-leap target year becomes 28 February.
                let day = if c.month == 2 && c.day == 29 {
                    28
                } else {
                    c.day
                };
                epoch(
                    c.year - i64::from(n),
                    c.month,
                    day,
                    c.hour,
                    c.minute,
                    c.second,
                )
            }
        }
    }

    pub fn valid(self) -> bool {
        self.amount() >= 1
            && match self {
                Retention::Days(n) => n <= MAX_RETENTION_DAYS,
                Retention::CalendarYears(n) => n <= 100,
            }
    }
}

/// What happens to a write whose event time is already in a closed bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LateWrites {
    RecomputeAffected,
    AcceptIntoOpenBuckets,
    Quarantine,
    Reject,
}

impl LateWrites {
    pub const ALL: [LateWrites; 4] = [
        LateWrites::RecomputeAffected,
        LateWrites::AcceptIntoOpenBuckets,
        LateWrites::Quarantine,
        LateWrites::Reject,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LateWrites::RecomputeAffected => "Recompute affected buckets",
            LateWrites::AcceptIntoOpenBuckets => "Accept into open buckets",
            LateWrites::Quarantine => "Quarantine",
            LateWrites::Reject => "Reject",
        }
    }
}

/// What happens to a write past the correction horizon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeyondHorizon {
    Reject,
    Quarantine,
    Accept,
    ScheduleRecompute,
}

impl BeyondHorizon {
    pub const ALL: [BeyondHorizon; 4] = [
        BeyondHorizon::Reject,
        BeyondHorizon::Quarantine,
        BeyondHorizon::Accept,
        BeyondHorizon::ScheduleRecompute,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BeyondHorizon::Reject => "Reject",
            BeyondHorizon::Quarantine => "Quarantine",
            BeyondHorizon::Accept => "Accept without recompute",
            BeyondHorizon::ScheduleRecompute => "Schedule a manual recompute",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LatePolicy {
    /// How far back a late write still corrects its buckets.
    pub horizon_days: u32,
    pub within: LateWrites,
    pub beyond: BeyondHorizon,
}

impl LatePolicy {
    pub fn issue(&self) -> Option<&'static str> {
        if self.horizon_days == 0 || self.horizon_days > MAX_RETENTION_DAYS {
            Some("Set a correction horizon of at least one day.")
        } else {
            None
        }
    }
}

/// One row of the retention table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tier {
    pub name: String,
    pub input: Option<String>,
    /// `None` for the raw collection.
    pub bucket: Option<Bucket>,
    pub keep: Retention,
    pub refresh: Option<RefreshPolicy>,
}

/// A collection's lifecycle as the catalog holds it.
#[derive(Debug, Clone, PartialEq)]
pub struct Lifecycle {
    pub collection: String,
    pub time_field: String,
    pub partition: String,
    pub raw_keep: Retention,
    /// Where expired raw rows go. `None` is not configured.
    pub archive: Option<String>,
    pub rollups: Vec<RollupDef>,
    pub late: LatePolicy,
    /// Columns a rollup can sum, and columns it can group by.
    pub numeric_fields: Vec<String>,
    pub group_fields: Vec<String>,
    /// Counts applied changes. A review names the revision it read.
    pub revision: u64,
}

impl Lifecycle {
    /// The raw collection first, then each rollup.
    pub fn tiers(&self) -> Vec<Tier> {
        let mut tiers = vec![Tier {
            name: self.collection.clone(),
            input: None,
            bucket: None,
            keep: self.raw_keep,
            refresh: None,
        }];
        tiers.extend(self.rollups.iter().map(|r| Tier {
            name: r.name.clone(),
            input: Some(r.input.clone()),
            bucket: Some(r.bucket),
            keep: r.keep,
            refresh: Some(r.refresh.clone()),
        }));
        tiers
    }

    pub fn rollup(&self, name: &str) -> Option<&RollupDef> {
        self.rollups.iter().find(|r| r.name == name)
    }

    /// The raw collection and every rollup, as a rollup input.
    pub fn inputs(&self) -> Vec<String> {
        std::iter::once(self.collection.clone())
            .chain(self.rollups.iter().map(|r| r.name.clone()))
            .collect()
    }

    /// Rollups cut, directly or not, from `name`.
    pub fn dependents(&self, name: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut frontier = vec![name.to_string()];
        while let Some(n) = frontier.pop() {
            for r in self.rollups.iter().filter(|r| r.input == n) {
                if !out.contains(&r.name) {
                    out.push(r.name.clone());
                    frontier.push(r.name.clone());
                }
            }
        }
        out
    }
}

/// What a draft of retention changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetentionDraft {
    pub raw_keep: Retention,
    /// Each rollup's retention, by name.
    pub rollup_keep: Vec<(String, Retention)>,
}

impl RetentionDraft {
    pub fn from(l: &Lifecycle) -> Self {
        Self {
            raw_keep: l.raw_keep,
            rollup_keep: l.rollups.iter().map(|r| (r.name.clone(), r.keep)).collect(),
        }
    }

    pub fn changes(&self, l: &Lifecycle) -> bool {
        self.raw_keep != l.raw_keep
            || self
                .rollup_keep
                .iter()
                .any(|(n, k)| l.rollup(n).is_some_and(|r| r.keep != *k))
    }

    pub fn issue(&self) -> Option<String> {
        if !self.raw_keep.valid() {
            return Some("Raw retention must be at least 1 and at most 100 years.".to_string());
        }
        self.rollup_keep
            .iter()
            .find(|(_, k)| !k.valid())
            .map(|(n, _)| format!("Retention of {n} must be at least 1 and at most 100 years."))
    }
}

/// One change waiting for review.
#[derive(Debug, Clone, PartialEq)]
pub enum LifecycleChange {
    Retention(RetentionDraft),
    CreateRollup(RollupDraft),
    LatePolicy(LatePolicy),
}

/// One change as the review shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewItem {
    pub title: String,
    pub sql: String,
    /// What the change does to data and to dependent tiers.
    pub impact: Vec<String>,
    /// The change deletes data that cannot be restored.
    pub irreversible: bool,
}

/// A review of pending changes, bound to the revision it read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeReview {
    pub revision: u64,
    pub items: Vec<ReviewItem>,
    /// Reasons the changes cannot be applied. Empty means they can.
    pub blocked: Vec<String>,
}

/// What an applied change leaves behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    /// The audit identifier of the operation.
    pub operation: String,
    pub applied: usize,
    pub revision: u64,
}

/// A refresh a person queued. Progress is not reported: the server gives
/// no measured progress, so none is shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollupRun {
    pub operation: String,
    pub rollup: String,
    pub requested: i64,
    pub status: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out(func: RollupFn, column: Option<&str>, alias: &str) -> RollupOutput {
        RollupOutput {
            func,
            column: column.map(str::to_string),
            alias: alias.into(),
        }
    }

    fn hourly() -> RollupDef {
        RollupDef {
            name: "sales_hourly".into(),
            input: "sales".into(),
            bucket: Bucket::Hour1,
            outputs: vec![
                out(RollupFn::Sum, Some("revenue"), "revenue"),
                out(RollupFn::Count, None, "sale_count"),
            ],
            group_by: vec!["store_id".into(), "currency".into()],
            refresh: RefreshPolicy::Interval("1m".into()),
            keep: Retention::Days(365),
        }
    }

    fn lifecycle() -> Lifecycle {
        Lifecycle {
            collection: "sales".into(),
            time_field: "sold_at".into(),
            partition: "1d".into(),
            raw_keep: Retention::Days(90),
            archive: None,
            rollups: vec![hourly()],
            late: LatePolicy {
                horizon_days: 7,
                within: LateWrites::RecomputeAffected,
                beyond: BeyondHorizon::Reject,
            },
            numeric_fields: vec!["revenue".into(), "quantity".into()],
            group_fields: vec!["store_id".into(), "product_id".into(), "currency".into()],
            revision: 1,
        }
    }

    #[test]
    fn calendar_years_are_not_365_days() {
        let now = epoch(2026, 6, 13, 13, 40, 0);
        assert_eq!(
            Retention::CalendarYears(3).cutoff(now),
            epoch(2023, 6, 13, 13, 40, 0)
        );
        // 2024 is a leap year: three calendar years back is 1096 days, not 1095.
        let days = |r: Retention| (now - r.cutoff(now)).div_euclid(86_400);
        assert_eq!(days(Retention::CalendarYears(3)), 1096);
        assert_eq!(days(Retention::Days(1095)), 1095);
        let leap = epoch(2028, 2, 29, 0, 0, 0);
        assert_eq!(
            Retention::CalendarYears(1).cutoff(leap),
            epoch(2027, 2, 28, 0, 0, 0)
        );
    }

    #[test]
    fn retention_reads_as_its_label() {
        assert_eq!(Retention::Days(90).label(), "90d");
        assert_eq!(Retention::CalendarYears(3).label(), "3y");
    }

    #[test]
    fn tiers_list_the_raw_collection_first() {
        let t = lifecycle().tiers();
        assert_eq!(t[0].name, "sales");
        assert_eq!(t[0].bucket, None);
        assert_eq!(t[1].name, "sales_hourly");
    }

    #[test]
    fn dependents_follow_the_chain() {
        let mut l = lifecycle();
        let mut daily = hourly();
        daily.name = "sales_daily".into();
        daily.input = "sales_hourly".into();
        l.rollups.push(daily);
        assert_eq!(l.dependents("sales"), ["sales_hourly", "sales_daily"]);
        assert_eq!(l.dependents("sales_daily"), Vec::<String>::new());
    }

    #[test]
    fn an_unchanged_retention_draft_changes_nothing() {
        let l = lifecycle();
        let mut d = RetentionDraft::from(&l);
        assert!(!d.changes(&l));
        d.raw_keep = Retention::Days(30);
        assert!(d.changes(&l));
    }

    #[test]
    fn a_zero_horizon_is_refused() {
        let mut p = lifecycle().late;
        assert!(p.issue().is_none());
        p.horizon_days = 0;
        assert!(p.issue().is_some());
    }
}
