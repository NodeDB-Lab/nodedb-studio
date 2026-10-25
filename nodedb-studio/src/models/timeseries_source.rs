//! Timeseries catalog: the collections the Timeseries pages can read, their
//! fields, and the checks of a query against one of them.

use crate::models::collection::Profile;
use crate::models::records::Filter;
use crate::models::timeseries::{
    Aggregate, Bucket, ExploreQuery, GapPolicy, MAX_BUCKETS, Measure, TimeSpan, is_identifier,
};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::format::epoch;

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
