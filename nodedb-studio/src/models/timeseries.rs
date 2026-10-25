//! Timeseries exploration: what a user asks of a collection over time, and
//! the shape of the answer.
//!
//! A query names a collection, an event-time field, one measure, a bucket,
//! an optional grouping field, filters, and a half-open UTC range
//! `[start, end)`. The answer is a set of series of buckets. A bucket the
//! source has no rows for is absent, never a zero: only an explicit gap
//! policy adds points, and each added point says it was filled.

use crate::models::format::{civil, date_second, epoch};
use crate::models::records::Filter;

/// The most buckets one query may span. A wider range or finer bucket is
/// refused before it runs.
pub const MAX_BUCKETS: usize = 2_000;

/// The aggregate a measure applies to the rows of one bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Aggregate {
    Count,
    Sum,
    Avg,
    Min,
    Max,
}

impl Aggregate {
    pub const ALL: [Aggregate; 5] = [
        Aggregate::Count,
        Aggregate::Sum,
        Aggregate::Avg,
        Aggregate::Min,
        Aggregate::Max,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Aggregate::Count => "COUNT",
            Aggregate::Sum => "SUM",
            Aggregate::Avg => "AVG",
            Aggregate::Min => "MIN",
            Aggregate::Max => "MAX",
        }
    }
}

/// A fixed-duration bucket, aligned to the UTC epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bucket {
    Minute1,
    Minute5,
    Minute15,
    Hour1,
    Hour6,
    Day1,
}

impl Bucket {
    pub const ALL: [Bucket; 6] = [
        Bucket::Minute1,
        Bucket::Minute5,
        Bucket::Minute15,
        Bucket::Hour1,
        Bucket::Hour6,
        Bucket::Day1,
    ];

    pub fn seconds(self) -> i64 {
        match self {
            Bucket::Minute1 => 60,
            Bucket::Minute5 => 300,
            Bucket::Minute15 => 900,
            Bucket::Hour1 => 3_600,
            Bucket::Hour6 => 21_600,
            Bucket::Day1 => 86_400,
        }
    }

    /// The interval text SQL and the toolbar use.
    pub fn label(self) -> &'static str {
        match self {
            Bucket::Minute1 => "1m",
            Bucket::Minute5 => "5m",
            Bucket::Minute15 => "15m",
            Bucket::Hour1 => "1h",
            Bucket::Hour6 => "6h",
            Bucket::Day1 => "1d",
        }
    }

    pub fn from_label(text: &str) -> Option<Bucket> {
        Bucket::ALL.into_iter().find(|b| b.label() == text)
    }
}

/// What a missing bucket becomes. Only `Missing` leaves it absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GapPolicy {
    #[default]
    Missing,
    Zero,
    CarryForward,
    Linear,
}

impl GapPolicy {
    pub const ALL: [GapPolicy; 4] = [
        GapPolicy::Missing,
        GapPolicy::Zero,
        GapPolicy::CarryForward,
        GapPolicy::Linear,
    ];

    pub fn label(self) -> &'static str {
        match self {
            GapPolicy::Missing => "Missing",
            GapPolicy::Zero => "Zero",
            GapPolicy::CarryForward => "Carry forward",
            GapPolicy::Linear => "Linear",
        }
    }

    /// One line on what the policy adds to the result.
    pub fn hint(self) -> &'static str {
        match self {
            GapPolicy::Missing => "Absent buckets stay gaps. Nothing is added.",
            GapPolicy::Zero => "Absent buckets show 0. The stored rows are unchanged.",
            GapPolicy::CarryForward => {
                "Absent buckets repeat the last observed value after the first observation."
            }
            GapPolicy::Linear => {
                "Absent buckets between two observed values lie on the line between them."
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ChartKind {
    #[default]
    Line,
    Bar,
    Area,
}

impl ChartKind {
    pub const ALL: [ChartKind; 3] = [ChartKind::Line, ChartKind::Bar, ChartKind::Area];

    pub fn label(self) -> &'static str {
        match self {
            ChartKind::Line => "Line",
            ChartKind::Bar => "Bar",
            ChartKind::Area => "Area",
        }
    }
}

/// A half-open UTC range `[start, end)` in epoch seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeSpan {
    pub start: i64,
    pub end: i64,
}

impl TimeSpan {
    pub const fn new(start: i64, end: i64) -> Self {
        Self { start, end }
    }

    pub fn contains(self, t: i64) -> bool {
        t >= self.start && t < self.end
    }

    /// The bucket starts that overlap the span, oldest first.
    pub fn bucket_starts(self, bucket: Bucket) -> Vec<i64> {
        let size = bucket.seconds();
        if self.end <= self.start {
            return Vec::new();
        }
        let first = self.start.div_euclid(size) * size;
        let mut at = first;
        let mut out = Vec::new();
        while at < self.end && out.len() <= MAX_BUCKETS {
            out.push(at);
            at += size;
        }
        out
    }
}

/// A range cut back from the instant the catalog was read. The cut is
/// frozen when it is chosen, so every query that shares it reads the same
/// window; choosing it again re-cuts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Last24Hours,
    Last7Days,
    Last30Days,
}

impl Preset {
    pub const ALL: [Preset; 3] = [Preset::Last24Hours, Preset::Last7Days, Preset::Last30Days];

    pub fn label(self) -> &'static str {
        match self {
            Preset::Last24Hours => "Last 24 hours",
            Preset::Last7Days => "Last 7 days",
            Preset::Last30Days => "Last 30 days",
        }
    }

    /// The range ending at the next whole hour (24 hours) or the end of the
    /// day (days) after `now`: the current bucket is inside it.
    pub fn span(self, now: i64) -> TimeSpan {
        let (unit, len) = match self {
            Preset::Last24Hours => (3_600, 24 * 3_600),
            Preset::Last7Days => (86_400, 7 * 86_400),
            Preset::Last30Days => (86_400, 30 * 86_400),
        };
        let end = now.div_euclid(unit) * unit + unit;
        TimeSpan::new(end - len, end)
    }
}

/// Parses `2026-06-07 00:00` (or `2026-06-07`) as a UTC instant.
pub fn parse_utc(text: &str) -> Option<i64> {
    let text = text.trim();
    let (day, clock) = match text.split_once(' ') {
        Some((d, c)) => (d, c.trim()),
        None => (text, "00:00"),
    };
    let mut d = day.split('-');
    let (y, mo, da) = (
        d.next()?.parse::<i64>().ok()?,
        d.next()?.parse::<u32>().ok()?,
        d.next()?.parse::<u32>().ok()?,
    );
    if d.next().is_some() || !(1..=12).contains(&mo) || !(1..=31).contains(&da) {
        return None;
    }
    let mut c = clock.split(':');
    let (h, mi) = (
        c.next()?.parse::<u32>().ok()?,
        c.next()?.parse::<u32>().ok()?,
    );
    if c.next().is_some() || h > 23 || mi > 59 {
        return None;
    }
    let at = epoch(y, mo, da, h, mi, 0);
    // A day that does not exist (31 June) rolls over: reject it.
    let back = civil(at, 0);
    (back.month == mo && back.day == da).then_some(at)
}

/// What one series plots: an aggregate of one field, or of every row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Measure {
    pub aggregate: Aggregate,
    /// `None` is `COUNT(*)`.
    pub field: Option<String>,
}

impl Measure {
    pub fn count_rows() -> Self {
        Self {
            aggregate: Aggregate::Count,
            field: None,
        }
    }

    pub fn of(aggregate: Aggregate, field: &str) -> Self {
        Self {
            aggregate,
            field: Some(field.to_string()),
        }
    }

    /// `SUM(revenue)` or `COUNT(*)`.
    pub fn label(&self) -> String {
        format!(
            "{}({})",
            self.aggregate.label(),
            self.field.as_deref().unwrap_or("*")
        )
    }

    /// The output column name.
    pub fn alias(&self) -> String {
        self.field.clone().unwrap_or_else(|| "rows".to_string())
    }
}

/// One question put to a collection.
#[derive(Debug, Clone, PartialEq)]
pub struct ExploreQuery {
    pub collection: String,
    pub time_field: String,
    pub measure: Measure,
    pub filters: Vec<Filter>,
    pub bucket: Bucket,
    pub group_by: Option<String>,
    pub gap: GapPolicy,
    pub span: TimeSpan,
}

/// A name SQL accepts without quoting.
pub fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A string literal, with its quotes doubled.
pub fn sql_literal(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

impl ExploreQuery {
    /// The SQL this query runs. Values are typed literals, never spliced
    /// fragments: a filter value cannot add a clause.
    pub fn sql(&self) -> String {
        let mut select = format!(
            "SELECT time_bucket('{}', {}) AS bucket,\n       ",
            self.bucket.label(),
            self.time_field
        );
        if let Some(g) = &self.group_by {
            select.push_str(g);
            select.push_str(", ");
        }
        select.push_str(&format!(
            "{} AS {}\nFROM {}\n",
            self.measure.label(),
            self.measure.alias(),
            self.collection
        ));
        let mut sql = select;
        sql.push_str(&format!(
            "WHERE {t} >= TIMESTAMP '{a}'\n  AND {t} < TIMESTAMP '{b}'",
            t = self.time_field,
            a = date_second(self.span.start, 0),
            b = date_second(self.span.end, 0)
        ));
        for f in &self.filters {
            sql.push_str(&format!("\n  AND {} = {}", f.field, sql_literal(&f.value)));
        }
        match &self.group_by {
            Some(g) => sql.push_str(&format!("\nGROUP BY bucket, {g}\nORDER BY bucket, {g};")),
            None => sql.push_str("\nGROUP BY bucket\nORDER BY bucket;"),
        }
        sql
    }

    /// `SUM(revenue) · 1d · USD only`.
    pub fn summary(&self) -> String {
        let mut parts = vec![self.measure.label(), self.bucket.label().to_string()];
        for f in &self.filters {
            parts.push(format!("{} only", f.value));
        }
        parts.join(" · ")
    }

    /// The chart's title: `Revenue by store_id`, or the measure alone.
    pub fn title(&self) -> String {
        let name = match &self.measure.field {
            Some(f) => {
                let mut c = f.chars();
                c.next()
                    .map(|first| first.to_uppercase().collect::<String>() + c.as_str())
                    .unwrap_or_default()
            }
            None => "Rows".to_string(),
        };
        match &self.group_by {
            Some(g) => format!("{name} by {g}"),
            None => name,
        }
    }
}

pub use crate::models::timeseries_result::{
    ExploreResult, Fill, Point, PointState, ResultRow, Series,
};
pub use crate::models::timeseries_source::{
    FieldInfo, IssueField, MoneyField, QueryIssue, TimeCatalog, TimeField, TimeSource, Unsupported,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn query() -> ExploreQuery {
        ExploreQuery {
            collection: "sales".into(),
            time_field: "sold_at".into(),
            measure: Measure::of(Aggregate::Sum, "revenue"),
            filters: vec![Filter {
                field: "currency".into(),
                value: "USD".into(),
            }],
            bucket: Bucket::Day1,
            group_by: Some("store_id".into()),
            gap: GapPolicy::Missing,
            span: TimeSpan::new(epoch(2026, 6, 7, 0, 0, 0), epoch(2026, 6, 14, 0, 0, 0)),
        }
    }

    #[test]
    fn sql_states_the_half_open_range_and_typed_filters() {
        let sql = query().sql();
        assert!(
            sql.contains("time_bucket('1d', sold_at) AS bucket"),
            "{sql}"
        );
        assert!(
            sql.contains("sold_at >= TIMESTAMP '2026-06-07 00:00:00'"),
            "{sql}"
        );
        assert!(
            sql.contains("sold_at < TIMESTAMP '2026-06-14 00:00:00'"),
            "{sql}"
        );
        assert!(sql.contains("AND currency = 'USD'"), "{sql}");
        assert!(sql.contains("GROUP BY bucket, store_id"), "{sql}");
    }

    #[test]
    fn a_filter_value_cannot_add_a_clause() {
        let mut q = query();
        q.filters[0].value = "USD' OR '1'='1".into();
        assert!(q.sql().contains("currency = 'USD'' OR ''1''=''1'"));
    }

    #[test]
    fn utc_text_parses_and_rejects_impossible_dates() {
        assert_eq!(
            parse_utc("2026-06-13 10:30"),
            Some(epoch(2026, 6, 13, 10, 30, 0))
        );
        assert_eq!(parse_utc("2026-06-13"), Some(epoch(2026, 6, 13, 0, 0, 0)));
        assert_eq!(parse_utc("2026-06-31 00:00"), None);
        assert_eq!(parse_utc("2026-06-13 24:00"), None);
        assert_eq!(parse_utc("June 13"), None);
    }

    #[test]
    fn presets_end_after_now_and_are_cut_once() {
        let now = epoch(2026, 6, 13, 13, 40, 0);
        let day = Preset::Last24Hours.span(now);
        assert_eq!(day.end, epoch(2026, 6, 13, 14, 0, 0));
        assert_eq!(day.end - day.start, 86_400);
        let week = Preset::Last7Days.span(now);
        assert_eq!(week.end, epoch(2026, 6, 14, 0, 0, 0));
        assert!(week.contains(now));
        assert_eq!(
            Preset::Last30Days.span(now).end - Preset::Last30Days.span(now).start,
            30 * 86_400
        );
    }

    #[test]
    fn bucket_starts_cover_a_misaligned_range() {
        let span = TimeSpan::new(epoch(2026, 6, 13, 10, 2, 0), epoch(2026, 6, 13, 10, 12, 0));
        let starts = span.bucket_starts(Bucket::Minute5);
        assert_eq!(starts.len(), 3);
        assert_eq!(starts[0], epoch(2026, 6, 13, 10, 0, 0));
    }
}
