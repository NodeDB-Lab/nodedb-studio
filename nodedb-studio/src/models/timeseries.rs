//! Timeseries exploration: what a user asks of a collection over time, and
//! the shape of the answer.
//!
//! A query names a collection, an event-time field, one measure, a bucket,
//! an optional grouping field, filters, and a half-open UTC range
//! `[start, end)`. The answer is a set of series of buckets. A bucket the
//! source has no rows for is absent, never a zero: only an explicit gap
//! policy adds points, and each added point says it was filled.

use crate::models::collection::Profile;
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

/// An event-time field a collection offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeField {
    pub name: String,
    /// The collection's declared TIME_KEY.
    pub time_key: bool,
}

/// One field a query can read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldInfo {
    pub name: String,
    pub numeric: bool,
    pub type_label: String,
}

/// Why a collection cannot be explored over time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsupported {
    NoTimestamp,
    KeyValue,
    NoRecords,
}

impl Unsupported {
    pub fn reason(self) -> &'static str {
        match self {
            Unsupported::NoTimestamp => {
                "No timestamp field. Add one in the Designer to explore it over time."
            }
            Unsupported::KeyValue => "A key-value collection has no event time.",
            Unsupported::NoRecords => "This collection holds no records to bucket.",
        }
    }
}

/// Every collection the database offers for temporal work, and the instant
/// the catalog was read. Relative ranges are cut from that instant, so every
/// query of one session shares it.
#[derive(Debug, Clone, PartialEq)]
pub struct TimeCatalog {
    pub observed_at: i64,
    pub sources: Vec<TimeSource>,
}

impl TimeCatalog {
    pub fn source(&self, collection: &str) -> Option<&TimeSource> {
        self.sources.iter().find(|s| s.collection == collection)
    }
}

/// An amount field and the field that names its currency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoneyField {
    pub field: String,
    pub currency_field: String,
    /// The currency a fresh query pins.
    pub currency: String,
}

/// A collection as the Timeseries pages see it: from the scoped catalog.
#[derive(Debug, Clone, PartialEq)]
pub struct TimeSource {
    pub collection: String,
    pub profile: Profile,
    pub time_fields: Vec<TimeField>,
    pub fields: Vec<FieldInfo>,
    /// Declared money fields. Never inferred from a name.
    pub money: Vec<MoneyField>,
    pub unsupported: Option<Unsupported>,
}

impl TimeSource {
    pub fn eligible(&self) -> bool {
        self.unsupported.is_none()
    }

    /// The declared time key first, then the other timestamp fields.
    pub fn default_time_field(&self) -> Option<&TimeField> {
        self.time_fields
            .iter()
            .find(|t| t.time_key)
            .or_else(|| self.time_fields.first())
    }

    pub fn numeric_fields(&self) -> Vec<&FieldInfo> {
        self.fields.iter().filter(|f| f.numeric).collect()
    }

    /// Text fields: the ones a result can group by.
    pub fn group_fields(&self) -> Vec<&FieldInfo> {
        self.fields
            .iter()
            .filter(|f| !f.numeric && !self.time_fields.iter().any(|t| t.name == f.name))
            .filter(|f| f.type_label.starts_with("string"))
            .collect()
    }

    pub fn field(&self, name: &str) -> Option<&FieldInfo> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// The currency field that gives `measure_field` its unit.
    pub fn currency_field(&self, measure_field: &str) -> Option<&str> {
        self.money
            .iter()
            .find(|m| m.field == measure_field)
            .map(|m| m.currency_field.as_str())
    }

    /// A fresh query over this collection: its time key, one-day buckets,
    /// over the seven days up to the end of `now`'s day. A collection with
    /// money sums its first amount in its declared currency, by its first
    /// text field. Any other counts rows.
    pub fn default_query(&self, now: i64) -> Option<ExploreQuery> {
        let time = self.default_time_field()?;
        let end = now.div_euclid(86_400) * 86_400 + 86_400;
        let money = self.money.first();
        Some(ExploreQuery {
            collection: self.collection.clone(),
            time_field: time.name.clone(),
            measure: money.map_or_else(Measure::count_rows, |m| {
                Measure::of(Aggregate::Sum, &m.field)
            }),
            filters: money
                .map(|m| {
                    vec![Filter {
                        field: m.currency_field.clone(),
                        value: m.currency.clone(),
                    }]
                })
                .unwrap_or_default(),
            bucket: Bucket::Day1,
            group_by: money
                .and_then(|m| {
                    self.group_fields()
                        .into_iter()
                        .find(|f| f.name != m.currency_field)
                })
                .map(|f| f.name.clone()),
            gap: GapPolicy::Missing,
            span: TimeSpan::new(end - 7 * 86_400, end),
        })
    }

    /// The currency a query pins through a filter on the measure's
    /// currency field.
    pub fn unit_of(&self, q: &ExploreQuery) -> Option<String> {
        let field = q.measure.field.as_deref()?;
        let currency = self.currency_field(field)?;
        q.filters
            .iter()
            .find(|f| f.field == currency)
            .map(|f| f.value.clone())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueField {
    Collection,
    TimeField,
    Measure,
    Filter,
    Group,
    Range,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryIssue {
    pub field: IssueField,
    pub message: String,
}

fn issue(field: IssueField, message: impl Into<String>) -> QueryIssue {
    QueryIssue {
        field,
        message: message.into(),
    }
}

impl ExploreQuery {
    /// Everything that stops this query from running, against the catalog
    /// entry of its collection. Empty means it can run.
    pub fn check(&self, source: &TimeSource) -> Vec<QueryIssue> {
        let mut out = Vec::new();
        if let Some(why) = source.unsupported {
            out.push(issue(IssueField::Collection, why.reason()));
            return out;
        }
        if !source.time_fields.iter().any(|t| t.name == self.time_field) {
            out.push(issue(
                IssueField::TimeField,
                format!(
                    "`{}` is not a timestamp field of {}.",
                    self.time_field, source.collection
                ),
            ));
        }
        match (&self.measure.field, self.measure.aggregate) {
            (None, Aggregate::Count) => {}
            (None, other) => out.push(issue(
                IssueField::Measure,
                format!("{} needs a field.", other.label()),
            )),
            (Some(f), agg) => match source.field(f) {
                None => out.push(issue(
                    IssueField::Measure,
                    format!("{} has no field `{f}`.", source.collection),
                )),
                Some(info) if !info.numeric && agg != Aggregate::Count => out.push(issue(
                    IssueField::Measure,
                    format!(
                        "{}({f}) needs a numeric field. `{f}` is {}.",
                        agg.label(),
                        info.type_label
                    ),
                )),
                Some(_) => {}
            },
        }
        if let Some(g) = &self.group_by
            && !source.group_fields().iter().any(|f| f.name == *g)
        {
            out.push(issue(
                IssueField::Group,
                format!("`{g}` cannot group {}.", source.collection),
            ));
        }
        for f in &self.filters {
            if !is_identifier(&f.field) || source.field(&f.field).is_none() {
                out.push(issue(
                    IssueField::Filter,
                    format!("{} has no field `{}`.", source.collection, f.field),
                ));
            } else if f.value.trim().is_empty() {
                out.push(issue(
                    IssueField::Filter,
                    format!("Give `{}` a value.", f.field),
                ));
            }
        }
        if self.span.end <= self.span.start {
            out.push(issue(
                IssueField::Range,
                "The range must end after it starts.",
            ));
        } else if self.span.bucket_starts(self.bucket).len() > MAX_BUCKETS {
            out.push(issue(
                IssueField::Range,
                format!(
                    "{} buckets exceed the budget of {MAX_BUCKETS}. Widen the bucket or shorten the range.",
                    self.span.bucket_starts(self.bucket).len()
                ),
            ));
        }
        // Money in two currencies never shares one axis.
        if let Some(field) = &self.measure.field
            && let Some(currency) = source.currency_field(field)
            && !self.filters.iter().any(|f| f.field == currency)
        {
            out.push(issue(
                IssueField::Filter,
                format!("`{field}` is money. Filter one `{currency}` so currencies do not share an axis."),
            ));
        }
        out
    }
}

/// How a point came to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fill {
    Zero,
    Carried,
    Interpolated,
}

impl Fill {
    pub fn label(self) -> &'static str {
        match self {
            Fill::Zero => "filled with zero",
            Fill::Carried => "carried forward",
            Fill::Interpolated => "interpolated",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointState {
    /// The bucket held rows and a value.
    Observed,
    /// The bucket held rows and every value was null.
    NullOnly,
    /// A gap policy added the point.
    Filled(Fill),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    /// The bucket start, epoch seconds.
    pub start: i64,
    pub value: Option<f64>,
    pub state: PointState,
    /// The bucket reaches past the observation time: it can still change.
    pub provisional: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    /// The group value. Empty when the query does not group.
    pub key: String,
    /// What the series ranks by: the sum of its values, or its row count
    /// for aggregates a sum does not describe.
    pub weight: f64,
    pub points: Vec<Point>,
}

impl Series {
    pub fn point_at(&self, start: i64) -> Option<&Point> {
        self.points.iter().find(|p| p.start == start)
    }
}

/// One row of the result table.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultRow {
    pub bucket: i64,
    pub group: String,
    pub value: Option<f64>,
    pub state: PointState,
}

/// One answer. Chart, table, and export all read this one value.
#[derive(Debug, Clone, PartialEq)]
pub struct ExploreResult {
    pub query: ExploreQuery,
    /// The instant the answer was computed at.
    pub observed_at: i64,
    /// Heaviest first. Never truncated here: a view that shows fewer says so.
    pub series: Vec<Series>,
    /// Every bucket start of the range, oldest first.
    pub buckets: Vec<i64>,
    /// Rows the source held in the range, before filters.
    pub rows_scanned: u64,
    /// Rows that passed the filters.
    pub rows_matched: u64,
    /// The pinned currency, when the measure is money.
    pub unit: Option<String>,
}

impl ExploreResult {
    /// Every point as a row, by bucket then group.
    pub fn rows(&self) -> Vec<ResultRow> {
        let mut rows: Vec<ResultRow> = self
            .series
            .iter()
            .flat_map(|s| {
                s.points.iter().map(|p| ResultRow {
                    bucket: p.start,
                    group: s.key.clone(),
                    value: p.value,
                    state: p.state,
                })
            })
            .collect();
        rows.sort_by(|a, b| a.bucket.cmp(&b.bucket).then_with(|| a.group.cmp(&b.group)));
        rows
    }

    pub fn is_empty(&self) -> bool {
        self.series.iter().all(|s| s.points.is_empty())
    }

    /// The rows of every series except those named in `hidden`.
    pub fn visible_rows(&self, hidden: &std::collections::BTreeSet<String>) -> Vec<ResultRow> {
        self.rows()
            .into_iter()
            .filter(|r| !hidden.contains(&r.group))
            .collect()
    }

    /// The rows as CSV: the bucket start in UTC, the group when the query
    /// groups, then the value. A null is an empty field and a filled point
    /// carries its provenance, so a copy never passes a fill off as stored.
    pub fn csv(&self, hidden: &std::collections::BTreeSet<String>) -> String {
        let quote = |t: &str| {
            if t.contains([',', '"', '\n']) {
                format!("\"{}\"", t.replace('"', "\"\""))
            } else {
                t.to_string()
            }
        };
        let mut head = vec!["bucket_utc".to_string()];
        if let Some(g) = &self.query.group_by {
            head.push(quote(g));
        }
        head.push(quote(&self.query.measure.alias()));
        head.push("state".to_string());
        let mut out = head.join(",");
        for r in self.visible_rows(hidden) {
            let mut cols = vec![date_second(r.bucket, 0)];
            if self.query.group_by.is_some() {
                cols.push(quote(&r.group));
            }
            cols.push(r.value.map(|v| v.to_string()).unwrap_or_default());
            cols.push(
                match r.state {
                    PointState::Observed => "observed",
                    PointState::NullOnly => "null",
                    PointState::Filled(f) => f.label(),
                }
                .to_string(),
            );
            out.push('\n');
            out.push_str(&cols.join(","));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sales_source() -> TimeSource {
        let f = |name: &str, numeric: bool, ty: &str| FieldInfo {
            name: name.into(),
            numeric,
            type_label: ty.into(),
        };
        TimeSource {
            collection: "sales".into(),
            profile: Profile::Timeseries,
            time_fields: vec![TimeField {
                name: "sold_at".into(),
                time_key: true,
            }],
            fields: vec![
                f("sold_at", false, "timestamp"),
                f("store_id", false, "string"),
                f("quantity", true, "int64"),
                f("revenue", true, "decimal(12,2)"),
                f("currency", false, "string(3)"),
            ],
            money: vec![MoneyField {
                field: "revenue".into(),
                currency_field: "currency".into(),
                currency: "USD".into(),
            }],
            unsupported: None,
        }
    }

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
    fn a_valid_query_has_no_issues() {
        assert!(query().check(&sales_source()).is_empty());
    }

    #[test]
    fn money_without_a_currency_filter_is_refused() {
        let mut q = query();
        q.filters.clear();
        let issues = q.check(&sales_source());
        assert!(issues.iter().any(|i| i.field == IssueField::Filter));
    }

    #[test]
    fn sum_of_text_is_refused_and_count_of_text_is_not() {
        let mut q = query();
        q.measure = Measure::of(Aggregate::Sum, "store_id");
        assert!(!q.check(&sales_source()).is_empty());
        q.measure = Measure::of(Aggregate::Count, "store_id");
        assert!(q.check(&sales_source()).is_empty());
    }

    #[test]
    fn a_range_over_the_bucket_budget_is_refused() {
        let mut q = query();
        q.bucket = Bucket::Minute1;
        assert!(
            q.check(&sales_source())
                .iter()
                .any(|i| i.field == IssueField::Range)
        );
    }

    #[test]
    fn an_empty_or_reversed_range_is_refused() {
        let mut q = query();
        q.span = TimeSpan::new(q.span.end, q.span.start);
        assert!(!q.check(&sales_source()).is_empty());
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
    fn csv_lists_utc_buckets_values_and_provenance() {
        let r = ExploreResult {
            query: query(),
            observed_at: 0,
            series: vec![
                Series {
                    key: "store, 1".into(),
                    weight: 1.0,
                    points: vec![
                        Point {
                            start: epoch(2026, 6, 7, 0, 0, 0),
                            value: Some(12.5),
                            state: PointState::Observed,
                            provisional: false,
                        },
                        Point {
                            start: epoch(2026, 6, 8, 0, 0, 0),
                            value: None,
                            state: PointState::NullOnly,
                            provisional: false,
                        },
                        Point {
                            start: epoch(2026, 6, 9, 0, 0, 0),
                            value: Some(0.0),
                            state: PointState::Filled(Fill::Zero),
                            provisional: false,
                        },
                    ],
                },
                Series {
                    key: "gone".into(),
                    weight: 0.0,
                    points: Vec::new(),
                },
            ],
            buckets: Vec::new(),
            rows_scanned: 0,
            rows_matched: 0,
            unit: None,
        };
        let csv = r.csv(&std::collections::BTreeSet::new());
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "bucket_utc,store_id,revenue,state");
        assert_eq!(lines[1], "2026-06-07 00:00:00,\"store, 1\",12.5,observed");
        assert_eq!(lines[2], "2026-06-08 00:00:00,\"store, 1\",,null");
        assert_eq!(
            lines[3],
            "2026-06-09 00:00:00,\"store, 1\",0,filled with zero"
        );
        let hidden: std::collections::BTreeSet<String> = ["store, 1".to_string()].into();
        assert_eq!(r.csv(&hidden).lines().count(), 1);
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

    #[test]
    fn the_default_query_ends_on_the_day_after_now() {
        let now = epoch(2026, 6, 13, 13, 40, 0);
        let q = sales_source().default_query(now).expect("query");
        assert_eq!(q.span.end, epoch(2026, 6, 14, 0, 0, 0));
        assert_eq!(q.span.end - q.span.start, 7 * 86_400);
    }

    #[test]
    fn the_default_query_of_a_money_collection_pins_its_currency() {
        let q = sales_source().default_query(0).expect("query");
        assert_eq!(q.measure.label(), "SUM(revenue)");
        assert_eq!(q.filters[0].label(), "currency = USD");
        assert_eq!(q.group_by.as_deref(), Some("store_id"));
        assert!(q.check(&sales_source()).is_empty());
    }

    #[test]
    fn the_default_query_of_any_other_collection_counts_rows() {
        let mut s = sales_source();
        s.money.clear();
        let q = s.default_query(0).expect("query");
        assert_eq!(q.measure.label(), "COUNT(*)");
        assert!(q.filters.is_empty() && q.group_by.is_none());
    }

    #[test]
    fn the_unit_comes_from_the_pinned_currency() {
        let s = sales_source();
        assert_eq!(s.unit_of(&query()).as_deref(), Some("USD"));
        let mut q = query();
        q.measure = Measure::count_rows();
        assert_eq!(s.unit_of(&q), None);
    }
}
