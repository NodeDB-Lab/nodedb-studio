//! Runs an `ExploreQuery` over the Explorer's records.

use nodedb_types::Value;

use super::sources::time_sources;
use crate::data::mock::explorer::{Overlay, all_records, observed_at, plain_text};
use crate::models::records::Record;
use crate::models::timeseries::{ExploreQuery, ExploreResult, QueryIssue};
use crate::models::timeseries_agg::{Sample, aggregate};

/// Why a query did not run.
#[derive(Debug, Clone, PartialEq)]
pub enum ExploreError {
    UnknownCollection(String),
    Invalid(Vec<QueryIssue>),
}

/// A value as a number. Decimals arrive as exact text.
fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Integer(i) => Some(*i as f64),
        Value::Float(f) => Some(*f),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn time_of(r: &Record, field: &str) -> Option<i64> {
    match r.get(field)? {
        Value::DateTime(t) | Value::NaiveDateTime(t) => Some(t.unix_secs()),
        _ => None,
    }
}

fn passes(r: &Record, q: &ExploreQuery) -> bool {
    q.filters.iter().all(|f| {
        r.get(&f.field)
            .is_some_and(|v| plain_text(v).eq_ignore_ascii_case(f.value.trim()))
    })
}

/// Answer `q` from the collection's records, as of the fixture instant.
pub fn explore(q: &ExploreQuery, overlay: &Overlay) -> Result<ExploreResult, ExploreError> {
    let source = time_sources()
        .into_iter()
        .find(|s| s.collection == q.collection)
        .ok_or_else(|| ExploreError::UnknownCollection(q.collection.clone()))?;
    let issues = q.check(&source);
    if !issues.is_empty() {
        return Err(ExploreError::Invalid(issues));
    }
    let records = all_records(&q.collection, overlay)
        .ok_or_else(|| ExploreError::UnknownCollection(q.collection.clone()))?;
    let (mut scanned, mut matched) = (0u64, 0u64);
    let mut samples = Vec::new();
    for r in &records {
        let Some(t) = time_of(r, &q.time_field) else {
            continue;
        };
        if !q.span.contains(t) {
            continue;
        }
        scanned += 1;
        if !passes(r, q) {
            continue;
        }
        matched += 1;
        samples.push(Sample {
            t,
            value: q
                .measure
                .field
                .as_deref()
                .and_then(|f| r.get(f))
                .and_then(number),
            group: q
                .group_by
                .as_deref()
                .and_then(|g| r.get(g))
                .map(plain_text)
                .unwrap_or_default(),
        });
    }
    let now = observed_at();
    Ok(ExploreResult {
        query: q.clone(),
        observed_at: now,
        series: aggregate(&samples, q, now),
        buckets: q.span.bucket_starts(q.bucket),
        rows_scanned: scanned,
        rows_matched: matched,
        unit: source.unit_of(q),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::format::epoch;
    use crate::models::records::Filter;
    use crate::models::timeseries::{Aggregate, Bucket, GapPolicy, Measure, TimeSpan};

    fn sales(measure: Measure, bucket: Bucket, group: Option<&str>) -> ExploreQuery {
        ExploreQuery {
            collection: "sales".into(),
            time_field: "sold_at".into(),
            measure,
            filters: vec![Filter {
                field: "currency".into(),
                value: "USD".into(),
            }],
            bucket,
            group_by: group.map(str::to_string),
            gap: GapPolicy::Missing,
            span: TimeSpan::new(epoch(2026, 6, 7, 0, 0, 0), epoch(2026, 6, 14, 0, 0, 0)),
        }
    }

    fn run(q: &ExploreQuery) -> ExploreResult {
        explore(q, &Overlay::default()).expect("runs")
    }

    #[test]
    fn counts_over_all_buckets_equal_the_rows_that_matched() {
        let r = run(&sales(Measure::count_rows(), Bucket::Day1, None));
        let total: f64 = r.series[0].points.iter().filter_map(|p| p.value).sum();
        assert_eq!(total as u64, r.rows_matched);
        assert!(r.rows_matched > 10_000);
        assert_eq!(r.rows_scanned, r.rows_matched, "every fixture row is USD");
    }

    /// The revenue of the ten authored rows, summed by hand.
    #[test]
    fn revenue_sums_exactly_for_a_known_hour() {
        let mut q = sales(Measure::of(Aggregate::Sum, "revenue"), Bucket::Day1, None);
        q.span = TimeSpan::new(epoch(2026, 6, 13, 0, 0, 0), epoch(2026, 6, 14, 0, 0, 0));
        let r = run(&q);
        // sal096..sal104: 29.99 + 124.95 + 14.99 + 39.98 + 79.96 + 9.99 + 49.98 + 19.99 + 59.97
        let want = 29.99 + 124.95 + 14.99 + 39.98 + 79.96 + 9.99 + 49.98 + 19.99 + 59.97;
        let got = r.series[0].points[0].value.expect("value");
        assert!((got - want).abs() < 1e-6, "{got} vs {want}");
        assert_eq!(r.rows_matched, 9);
        // 13 June is the observation day: its bucket is provisional.
        assert!(r.series[0].points[0].provisional);
    }

    #[test]
    fn grouping_by_store_makes_a_series_per_store_that_adds_up_to_the_whole() {
        let by_store = run(&sales(
            Measure::of(Aggregate::Sum, "revenue"),
            Bucket::Day1,
            Some("store_id"),
        ));
        let whole = run(&sales(
            Measure::of(Aggregate::Sum, "revenue"),
            Bucket::Day1,
            None,
        ));
        assert!(by_store.series.len() > 100);
        let sum_of_parts: f64 = by_store
            .series
            .iter()
            .flat_map(|s| s.points.iter().filter_map(|p| p.value))
            .sum();
        let total: f64 = whole.series[0].points.iter().filter_map(|p| p.value).sum();
        assert!((sum_of_parts - total).abs() < 1e-4);
        assert!(by_store.series[0].weight >= by_store.series[1].weight);
    }

    #[test]
    fn a_filter_that_matches_nothing_gives_an_empty_result() {
        let mut q = sales(Measure::count_rows(), Bucket::Day1, None);
        q.filters[0].value = "EUR".into();
        let r = run(&q);
        assert!(r.is_empty());
        assert_eq!(r.rows_matched, 0);
        assert!(r.rows_scanned > 0);
    }

    #[test]
    fn the_range_end_is_exclusive() {
        let mut q = sales(Measure::count_rows(), Bucket::Minute1, None);
        // sal104 sold at 13:21:14.
        q.span = TimeSpan::new(epoch(2026, 6, 13, 13, 21, 0), epoch(2026, 6, 13, 13, 22, 0));
        assert_eq!(run(&q).rows_matched, 1);
        q.span = TimeSpan::new(epoch(2026, 6, 13, 13, 20, 0), epoch(2026, 6, 13, 13, 21, 0));
        assert_eq!(run(&q).rows_matched, 0);
    }

    #[test]
    fn the_unit_is_the_pinned_currency() {
        let r = run(&sales(
            Measure::of(Aggregate::Sum, "revenue"),
            Bucket::Day1,
            None,
        ));
        assert_eq!(r.unit.as_deref(), Some("USD"));
    }

    #[test]
    fn an_invalid_query_is_refused_with_its_issues() {
        let mut q = sales(Measure::of(Aggregate::Sum, "revenue"), Bucket::Day1, None);
        q.filters.clear();
        assert!(matches!(
            explore(&q, &Overlay::default()),
            Err(ExploreError::Invalid(_))
        ));
        q.collection = "nope".into();
        assert!(matches!(
            explore(&q, &Overlay::default()),
            Err(ExploreError::UnknownCollection(_))
        ));
    }

    #[test]
    fn another_collection_buckets_on_a_field_the_user_chose() {
        let q = ExploreQuery {
            collection: "shipments".into(),
            time_field: "scheduled_at".into(),
            measure: Measure::count_rows(),
            filters: Vec::new(),
            bucket: Bucket::Day1,
            group_by: Some("status".into()),
            gap: GapPolicy::Missing,
            span: TimeSpan::new(epoch(2026, 6, 7, 0, 0, 0), epoch(2026, 6, 14, 0, 0, 0)),
        };
        let r = run(&q);
        assert!(r.series.len() >= 3);
        assert!(r.rows_matched > 100);
    }
}
