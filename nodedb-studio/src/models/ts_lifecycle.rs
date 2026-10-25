//! Lifecycle of a timeseries collection: raw retention, rollups, and the
//! late-data policy.
//!
//! A saved definition is not a runtime state. Nothing here says a rollup is
//! fresh, complete, or backfilled: those facts come from measured telemetry
//! and read `None` until the server reports them.

use crate::models::format::{civil, epoch};
use crate::models::timeseries::{Bucket, is_identifier};

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

/// When a rollup folds new rows in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshPolicy {
    /// A fixed interval such as `1m`.
    Interval(String),
    OnSeal,
    OnFlush,
    Manual,
}

impl RefreshPolicy {
    pub fn label(&self) -> String {
        match self {
            RefreshPolicy::Interval(i) => format!("Every {i}"),
            RefreshPolicy::OnSeal => "On seal".to_string(),
            RefreshPolicy::OnFlush => "On flush".to_string(),
            RefreshPolicy::Manual => "Manual".to_string(),
        }
    }

    /// The `WITH (...)` entry.
    pub fn sql(&self) -> String {
        match self {
            RefreshPolicy::Interval(i) => format!("refresh_interval='{i}'"),
            RefreshPolicy::OnSeal => "refresh_policy='on_seal'".to_string(),
            RefreshPolicy::OnFlush => "refresh_policy='on_flush'".to_string(),
            RefreshPolicy::Manual => "refresh_policy='manual'".to_string(),
        }
    }
}

/// An aggregate a rollup stores. Averages are not stored: store SUM and
/// COUNT, which merge exactly, and divide when reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RollupFn {
    Sum,
    Count,
    Min,
    Max,
}

impl RollupFn {
    pub const ALL: [RollupFn; 4] = [RollupFn::Sum, RollupFn::Count, RollupFn::Min, RollupFn::Max];

    pub fn sql(self) -> &'static str {
        match self {
            RollupFn::Sum => "sum",
            RollupFn::Count => "count",
            RollupFn::Min => "min",
            RollupFn::Max => "max",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            RollupFn::Sum => "SUM",
            RollupFn::Count => "COUNT",
            RollupFn::Min => "MIN",
            RollupFn::Max => "MAX",
        }
    }

    /// The function that merges stored values of this aggregate into a
    /// coarser bucket.
    fn merge(self) -> RollupFn {
        match self {
            RollupFn::Sum | RollupFn::Count => RollupFn::Sum,
            RollupFn::Min => RollupFn::Min,
            RollupFn::Max => RollupFn::Max,
        }
    }
}

/// One stored output of a rollup, in terms of the collection's columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollupOutput {
    pub func: RollupFn,
    /// `None` is `COUNT(*)`.
    pub column: Option<String>,
    pub alias: String,
}

impl RollupOutput {
    /// `SUM(revenue)` or `COUNT(*)`.
    pub fn logical(&self) -> String {
        format!(
            "{}({})",
            self.func.label(),
            self.column.as_deref().unwrap_or("*")
        )
    }

    /// `revenue = SUM(revenue)`.
    pub fn line(&self) -> String {
        format!("{} = {}", self.alias, self.logical())
    }
}

/// A saved continuous aggregate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollupDef {
    pub name: String,
    /// The collection, or the rollup this one is cut from.
    pub input: String,
    pub bucket: Bucket,
    pub outputs: Vec<RollupOutput>,
    pub group_by: Vec<String>,
    pub refresh: RefreshPolicy,
    pub keep: Retention,
}

/// The aggregate an output reads from `input`, when `input` is a rollup:
/// its stored output of the same function and column.
fn source_output<'a>(input: &'a RollupDef, o: &RollupOutput) -> Option<&'a RollupOutput> {
    input
        .outputs
        .iter()
        .find(|i| i.func == o.func && i.column == o.column)
}

impl RollupDef {
    /// The `CREATE CONTINUOUS AGGREGATE` statement. `input_def` is the
    /// input rollup when this one is cut from another: its outputs merge
    /// the stored ones, so a count becomes a sum of counts.
    pub fn sql(&self, collection_input: bool, input_def: Option<&RollupDef>) -> String {
        let outputs: Vec<String> = self
            .outputs
            .iter()
            .map(|o| {
                match (
                    collection_input,
                    input_def.and_then(|d| source_output(d, o)),
                ) {
                    (false, Some(src)) => {
                        format!("{}({}) AS {}", o.func.merge().sql(), src.alias, o.alias)
                    }
                    _ => match &o.column {
                        Some(c) => format!("{}({c}) AS {}", o.func.sql(), o.alias),
                        None => format!("count(*) AS {}", o.alias),
                    },
                }
            })
            .collect();
        let mut sql = format!(
            "CREATE CONTINUOUS AGGREGATE {}\nON {}\nBUCKET '{}'\nAGGREGATE {}",
            self.name,
            self.input,
            self.bucket.label(),
            outputs.join(",\n          ")
        );
        if !self.group_by.is_empty() {
            sql.push_str(&format!("\nGROUP BY {}", self.group_by.join(", ")));
        }
        sql.push_str(&format!(
            "\nWITH ({},\n      retention='{}');",
            self.refresh.sql(),
            self.keep.label()
        ));
        sql
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

/// A rollup the user is defining.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollupDraft {
    pub name: String,
    pub input: String,
    pub bucket: Bucket,
    pub outputs: Vec<RollupOutput>,
    pub group_by: Vec<String>,
    pub refresh: RefreshPolicy,
    pub keep: Retention,
    /// The user accepts a coarser grouping than the input has.
    pub allow_lossy: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftField {
    Name,
    Input,
    Bucket,
    Outputs,
    Group,
    Keep,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftIssue {
    pub field: DraftField,
    pub message: String,
}

fn bad(field: DraftField, message: impl Into<String>) -> DraftIssue {
    DraftIssue {
        field,
        message: message.into(),
    }
}

impl RollupDraft {
    /// A draft over the collection with the usual outputs, ready to edit.
    pub fn blank(l: &Lifecycle) -> Self {
        Self {
            name: String::new(),
            input: l.collection.clone(),
            bucket: Bucket::Hour1,
            outputs: Vec::new(),
            group_by: l.group_fields.clone(),
            refresh: RefreshPolicy::Interval("1m".to_string()),
            keep: Retention::Days(365),
            allow_lossy: false,
        }
    }

    pub fn def(&self) -> RollupDef {
        RollupDef {
            name: self.name.trim().to_string(),
            input: self.input.clone(),
            bucket: self.bucket,
            outputs: self.outputs.clone(),
            group_by: self.group_by.clone(),
            refresh: self.refresh.clone(),
            keep: self.keep,
        }
    }

    /// What stops this draft from becoming a definition.
    pub fn issues(&self, l: &Lifecycle) -> Vec<DraftIssue> {
        let mut out = Vec::new();
        let name = self.name.trim();
        if !is_identifier(name) {
            out.push(bad(
                DraftField::Name,
                "Name the rollup with letters, digits, and underscores.",
            ));
        } else if l.inputs().iter().any(|n| n == name) {
            out.push(bad(
                DraftField::Name,
                format!("`{name}` already exists in this database."),
            ));
        }
        let input_def = l.rollup(&self.input);
        let input_bucket = input_def.map(|d| d.bucket);
        if !l.inputs().contains(&self.input) {
            out.push(bad(
                DraftField::Input,
                format!("`{}` is not an input of {}.", self.input, l.collection),
            ));
        }
        if let Some(b) = input_bucket
            && (self.bucket.seconds() <= b.seconds() || self.bucket.seconds() % b.seconds() != 0)
        {
            out.push(bad(
                DraftField::Bucket,
                format!(
                    "{} must be a whole multiple of {}'s bucket of {}.",
                    self.bucket.label(),
                    self.input,
                    b.label()
                ),
            ));
        }
        if self.outputs.is_empty() {
            out.push(bad(DraftField::Outputs, "Store at least one output."));
        }
        for o in &self.outputs {
            if let Some(d) = input_def
                && source_output(d, o).is_none()
            {
                out.push(bad(
                    DraftField::Outputs,
                    format!(
                        "{} does not store {}. A rollup merges what its input stored.",
                        d.name,
                        o.logical()
                    ),
                ));
            }
            if o.func != RollupFn::Count && o.column.is_none() {
                out.push(bad(
                    DraftField::Outputs,
                    format!("{} needs a column.", o.func.label()),
                ));
            }
            if !is_identifier(&o.alias) {
                out.push(bad(
                    DraftField::Outputs,
                    format!("`{}` is not a valid output name.", o.alias),
                ));
            }
        }
        let mut aliases: Vec<&str> = self.outputs.iter().map(|o| o.alias.as_str()).collect();
        aliases.sort_unstable();
        if aliases.windows(2).any(|w| w[0] == w[1]) {
            out.push(bad(DraftField::Outputs, "Two outputs share a name."));
        }
        let available: Vec<String> = match input_def {
            Some(d) => d.group_by.clone(),
            None => l.group_fields.clone(),
        };
        if let Some(g) = self.group_by.iter().find(|g| !available.contains(g)) {
            out.push(bad(
                DraftField::Group,
                format!("`{g}` is not a dimension of {}.", self.input),
            ));
        }
        let dropped: Vec<&String> = available
            .iter()
            .filter(|a| !self.group_by.contains(a))
            .collect();
        if !dropped.is_empty() && !self.allow_lossy {
            out.push(bad(
                DraftField::Group,
                format!(
                    "Dropping {} merges series that differ in it, and the merge cannot be undone. Allow dropping dimensions to continue.",
                    dropped.iter().map(|d| d.as_str()).collect::<Vec<_>>().join(", ")
                ),
            ));
        }
        if !self.keep.valid() {
            out.push(bad(
                DraftField::Keep,
                "Retention must be at least 1 and at most 100 years.",
            ));
        }
        out
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

    fn draft() -> RollupDraft {
        RollupDraft {
            name: "sales_daily".into(),
            input: "sales_hourly".into(),
            bucket: Bucket::Day1,
            outputs: vec![
                out(RollupFn::Sum, Some("revenue"), "revenue"),
                out(RollupFn::Count, None, "sale_count"),
            ],
            group_by: vec!["store_id".into(), "currency".into()],
            refresh: RefreshPolicy::OnSeal,
            keep: Retention::CalendarYears(3),
            allow_lossy: false,
        }
    }

    #[test]
    fn the_hourly_definition_renders_as_the_server_syntax() {
        let sql = hourly().sql(true, None);
        assert!(sql.starts_with("CREATE CONTINUOUS AGGREGATE sales_hourly\nON sales\nBUCKET '1h'"));
        assert!(sql.contains("sum(revenue) AS revenue"));
        assert!(sql.contains("count(*) AS sale_count"));
        assert!(sql.contains("GROUP BY store_id, currency"));
        assert!(sql.ends_with("WITH (refresh_interval='1m',\n      retention='365d');"));
    }

    /// A daily count is the sum of the hourly counts, not a recount.
    #[test]
    fn a_chained_rollup_merges_what_its_input_stored() {
        let daily = draft().def();
        let sql = daily.sql(false, Some(&hourly()));
        assert!(sql.contains("sum(revenue) AS revenue"), "{sql}");
        assert!(sql.contains("sum(sale_count) AS sale_count"), "{sql}");
        assert!(sql.contains("ON sales_hourly"), "{sql}");
    }

    #[test]
    fn a_valid_chained_draft_has_no_issues() {
        assert!(draft().issues(&lifecycle()).is_empty());
    }

    #[test]
    fn a_name_in_use_is_refused() {
        let mut d = draft();
        d.name = "sales_hourly".into();
        assert!(
            d.issues(&lifecycle())
                .iter()
                .any(|i| i.field == DraftField::Name)
        );
        d.name = "sales".into();
        assert!(
            d.issues(&lifecycle())
                .iter()
                .any(|i| i.field == DraftField::Name)
        );
    }

    #[test]
    fn a_bucket_that_is_not_a_multiple_of_its_input_is_refused() {
        let mut d = draft();
        d.bucket = Bucket::Hour1;
        assert!(
            d.issues(&lifecycle())
                .iter()
                .any(|i| i.field == DraftField::Bucket)
        );
    }

    #[test]
    fn an_output_the_input_never_stored_is_refused() {
        let mut d = draft();
        d.outputs
            .push(out(RollupFn::Min, Some("revenue"), "min_revenue"));
        let issues = d.issues(&lifecycle());
        assert!(
            issues
                .iter()
                .any(|i| i.field == DraftField::Outputs && i.message.contains("MIN(revenue)"))
        );
    }

    #[test]
    fn dropping_a_dimension_needs_consent() {
        let mut d = draft();
        d.group_by = vec!["currency".into()];
        assert!(
            d.issues(&lifecycle())
                .iter()
                .any(|i| i.field == DraftField::Group)
        );
        d.allow_lossy = true;
        assert!(d.issues(&lifecycle()).is_empty());
    }

    #[test]
    fn a_dimension_the_input_lacks_is_refused() {
        let mut d = draft();
        d.group_by.push("product_id".into());
        assert!(
            d.issues(&lifecycle())
                .iter()
                .any(|i| i.field == DraftField::Group)
        );
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
        let mut daily = draft().def();
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
