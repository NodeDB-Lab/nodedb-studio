//! Bucketed aggregation over timestamped samples, in plain Rust.
//!
//! Each bucket keeps a mergeable partial: a row count, a count of non-null
//! values, their sum, and their minimum and maximum. An average is always
//! `sum / count` of merged partials, never a mean of means: two buckets of
//! different sizes do not weigh equally.

use std::collections::BTreeMap;

use crate::models::timeseries::{
    Aggregate, Bucket, ExploreQuery, Fill, GapPolicy, Point, PointState, Series,
};

/// One row reduced to what aggregation reads.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    /// The event time, epoch seconds.
    pub t: i64,
    /// The measure's value. `None` is a null.
    pub value: Option<f64>,
    /// The grouping value. Empty when the query does not group.
    pub group: String,
}

/// What one bucket of one group has seen.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Partial {
    /// Every row, null or not.
    pub rows: u64,
    /// Rows with a value.
    pub count: u64,
    pub sum: f64,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

impl Partial {
    pub fn add(&mut self, value: Option<f64>) {
        self.rows += 1;
        if let Some(v) = value {
            self.count += 1;
            self.sum += v;
            self.min = Some(self.min.map_or(v, |m| m.min(v)));
            self.max = Some(self.max.map_or(v, |m| m.max(v)));
        }
    }

    /// Fold `other` in. Merging partials of finer buckets gives the exact
    /// partial of the coarser bucket. The tests use it to show why an
    /// average is never a mean of means.
    #[cfg(test)]
    pub fn merge(&mut self, other: &Partial) {
        self.rows += other.rows;
        self.count += other.count;
        self.sum += other.sum;
        self.min = match (self.min, other.min) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        self.max = match (self.max, other.max) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
    }

    /// The aggregate's value. `COUNT(*)` counts rows. The others read only
    /// non-null values, and are null when a bucket has none.
    pub fn finish(&self, aggregate: Aggregate) -> Option<f64> {
        match aggregate {
            Aggregate::Count => Some(self.rows as f64),
            Aggregate::Sum => (self.count > 0).then_some(self.sum),
            Aggregate::Avg => (self.count > 0).then(|| self.sum / self.count as f64),
            Aggregate::Min => self.min,
            Aggregate::Max => self.max,
        }
    }
}

/// The start of the bucket `t` falls in.
pub fn bucket_start(t: i64, bucket: Bucket) -> i64 {
    let size = bucket.seconds();
    t.div_euclid(size) * size
}

/// Aggregate `samples` into series for `query`, as of `now`.
///
/// Samples outside `[start, end)` are ignored: the end is exclusive. Series
/// come back heaviest first.
pub fn aggregate(samples: &[Sample], query: &ExploreQuery, now: i64) -> Vec<Series> {
    let size = query.bucket.seconds();
    let mut groups: BTreeMap<&str, BTreeMap<i64, Partial>> = BTreeMap::new();
    for s in samples.iter().filter(|s| query.span.contains(s.t)) {
        groups
            .entry(s.group.as_str())
            .or_default()
            .entry(bucket_start(s.t, query.bucket))
            .or_default()
            .add(s.value);
    }
    let starts = query.span.bucket_starts(query.bucket);
    let mut series: Vec<Series> = groups
        .into_iter()
        .map(|(key, buckets)| {
            let observed: Vec<(i64, Partial)> = buckets.into_iter().collect();
            let weight = weight_of(&observed, query.measure.aggregate);
            let points = fill(&observed, &starts, query, now, size);
            Series {
                key: key.to_string(),
                weight,
                points,
            }
        })
        .collect();
    series.sort_by(|a, b| {
        b.weight
            .total_cmp(&a.weight)
            .then_with(|| a.key.cmp(&b.key))
    });
    series
}

/// What ranks a series: values that add up rank by their sum, the rest by
/// how many rows stand behind them.
fn weight_of(observed: &[(i64, Partial)], aggregate: Aggregate) -> f64 {
    match aggregate {
        Aggregate::Count | Aggregate::Sum => observed
            .iter()
            .filter_map(|(_, p)| p.finish(aggregate))
            .sum(),
        Aggregate::Avg | Aggregate::Min | Aggregate::Max => {
            observed.iter().map(|(_, p)| p.rows as f64).sum()
        }
    }
}

fn point(start: i64, p: &Partial, query: &ExploreQuery, now: i64, size: i64) -> Point {
    let value = p.finish(query.measure.aggregate);
    Point {
        start,
        value,
        state: if value.is_some() {
            PointState::Observed
        } else {
            PointState::NullOnly
        },
        provisional: start + size > now,
    }
}

/// The series' points under the query's gap policy. Observed points are
/// never altered: a policy only adds points, each marked as filled.
fn fill(
    observed: &[(i64, Partial)],
    starts: &[i64],
    query: &ExploreQuery,
    now: i64,
    size: i64,
) -> Vec<Point> {
    let by_start: BTreeMap<i64, &Partial> = observed.iter().map(|(s, p)| (*s, p)).collect();
    let real = |s: i64| by_start.get(&s).map(|p| point(s, p, query, now, size));
    if query.gap == GapPolicy::Missing {
        return starts.iter().filter_map(|s| real(*s)).collect();
    }
    let mut out: Vec<Point> = Vec::with_capacity(starts.len());
    // The last bucket that held a value, for carrying forward.
    let mut last: Option<f64> = None;
    for (i, s) in starts.iter().enumerate() {
        if let Some(p) = real(*s) {
            if p.value.is_some() {
                last = p.value;
            }
            out.push(p);
            continue;
        }
        let filled = match query.gap {
            GapPolicy::Missing => None,
            GapPolicy::Zero => Some((0.0, Fill::Zero)),
            GapPolicy::CarryForward => last.map(|v| (v, Fill::Carried)),
            GapPolicy::Linear => interpolate(i, starts, &real).map(|v| (v, Fill::Interpolated)),
        };
        if let Some((v, how)) = filled {
            out.push(Point {
                start: *s,
                value: Some(v),
                state: PointState::Filled(how),
                provisional: false,
            });
        }
    }
    out
}

/// The value at `starts[i]` on the line between the nearest observed values
/// on either side. `None` at either edge, where there is nothing to join.
fn interpolate(i: usize, starts: &[i64], real: &impl Fn(i64) -> Option<Point>) -> Option<f64> {
    let valued = |s: i64| real(s).and_then(|p| p.value);
    let before = (0..i)
        .rev()
        .find_map(|j| valued(starts[j]).map(|v| (starts[j], v)))?;
    let after = (i + 1..starts.len()).find_map(|j| valued(starts[j]).map(|v| (starts[j], v)))?;
    let span = (after.0 - before.0) as f64;
    let along = (starts[i] - before.0) as f64 / span;
    Some(before.1 + (after.1 - before.1) * along)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::format::epoch;
    use crate::models::records::Filter;
    use crate::models::timeseries::{Measure, TimeSpan};

    const NOW: i64 = epoch(2026, 6, 13, 13, 40, 0);

    fn at(h: u32, m: u32) -> i64 {
        epoch(2026, 6, 13, h, m, 0)
    }

    fn sample(t: i64, v: Option<f64>, g: &str) -> Sample {
        Sample {
            t,
            value: v,
            group: g.into(),
        }
    }

    fn query(aggregate: Aggregate, bucket: Bucket, gap: GapPolicy) -> ExploreQuery {
        ExploreQuery {
            collection: "sales".into(),
            time_field: "sold_at".into(),
            measure: Measure {
                aggregate,
                field: (aggregate != Aggregate::Count).then(|| "revenue".to_string()),
            },
            filters: Vec::<Filter>::new(),
            bucket,
            group_by: None,
            gap,
            span: TimeSpan::new(at(10, 0), at(11, 0)),
        }
    }

    fn values(s: &Series) -> Vec<(i64, Option<f64>)> {
        s.points.iter().map(|p| (p.start, p.value)).collect()
    }

    #[test]
    fn sum_and_count_add_up_per_bucket() {
        let rows = [
            sample(at(10, 1), Some(10.0), ""),
            sample(at(10, 4), Some(5.5), ""),
            sample(at(10, 6), Some(2.0), ""),
        ];
        let q = query(Aggregate::Sum, Bucket::Minute5, GapPolicy::Missing);
        let s = aggregate(&rows, &q, NOW);
        assert_eq!(
            values(&s[0]),
            [(at(10, 0), Some(15.5)), (at(10, 5), Some(2.0))]
        );
        let q = query(Aggregate::Count, Bucket::Minute5, GapPolicy::Missing);
        let s = aggregate(&rows, &q, NOW);
        assert_eq!(
            values(&s[0]),
            [(at(10, 0), Some(2.0)), (at(10, 5), Some(1.0))]
        );
    }

    /// Average is sum over count, not the mean of the bucket means.
    #[test]
    fn the_average_of_a_coarse_bucket_is_weighted_by_rows() {
        let rows = [
            // Bucket 10:00 holds one row, bucket 10:05 holds three.
            sample(at(10, 0), Some(100.0), ""),
            sample(at(10, 5), Some(10.0), ""),
            sample(at(10, 6), Some(10.0), ""),
            sample(at(10, 7), Some(10.0), ""),
        ];
        let fine = aggregate(
            &rows,
            &query(Aggregate::Avg, Bucket::Minute5, GapPolicy::Missing),
            NOW,
        );
        let means: Vec<f64> = fine[0].points.iter().filter_map(|p| p.value).collect();
        assert_eq!(means, [100.0, 10.0]);
        let mean_of_means = means.iter().sum::<f64>() / means.len() as f64;

        // The right coarse average merges the partials.
        let mut merged = Partial::default();
        for v in [100.0, 10.0, 10.0, 10.0] {
            let mut p = Partial::default();
            p.add(Some(v));
            merged.merge(&p);
        }
        let weighted = merged.finish(Aggregate::Avg).expect("average");
        assert_eq!(weighted, 32.5);
        assert_ne!(weighted, mean_of_means);

        let coarse = aggregate(
            &rows,
            &query(Aggregate::Avg, Bucket::Hour1, GapPolicy::Missing),
            NOW,
        );
        assert_eq!(coarse[0].points[0].value, Some(32.5));
    }

    #[test]
    fn merging_fine_partials_equals_the_coarse_partial() {
        let rows = [
            sample(at(10, 1), Some(4.0), ""),
            sample(at(10, 9), Some(8.0), ""),
            sample(at(10, 12), None, ""),
        ];
        let fine: Vec<Partial> = rows
            .iter()
            .map(|r| {
                let mut p = Partial::default();
                p.add(r.value);
                p
            })
            .collect();
        let mut merged = Partial::default();
        fine.iter().for_each(|p| merged.merge(p));
        let mut whole = Partial::default();
        rows.iter().for_each(|r| whole.add(r.value));
        assert_eq!(merged, whole);
    }

    #[test]
    fn min_and_max_ignore_nulls() {
        let rows = [
            sample(at(10, 1), Some(4.0), ""),
            sample(at(10, 2), None, ""),
            sample(at(10, 3), Some(9.0), ""),
        ];
        let min = aggregate(
            &rows,
            &query(Aggregate::Min, Bucket::Hour1, GapPolicy::Missing),
            NOW,
        );
        let max = aggregate(
            &rows,
            &query(Aggregate::Max, Bucket::Hour1, GapPolicy::Missing),
            NOW,
        );
        assert_eq!(min[0].points[0].value, Some(4.0));
        assert_eq!(max[0].points[0].value, Some(9.0));
    }

    #[test]
    fn count_all_rows_counts_nulls_but_sum_does_not() {
        let rows = [
            sample(at(10, 1), None, ""),
            sample(at(10, 2), Some(3.0), ""),
        ];
        let count = aggregate(
            &rows,
            &query(Aggregate::Count, Bucket::Hour1, GapPolicy::Missing),
            NOW,
        );
        assert_eq!(count[0].points[0].value, Some(2.0));
        let sum = aggregate(
            &rows,
            &query(Aggregate::Sum, Bucket::Hour1, GapPolicy::Missing),
            NOW,
        );
        assert_eq!(sum[0].points[0].value, Some(3.0));
    }

    /// A bucket of nulls is observed with no value: not absent, not zero.
    #[test]
    fn a_bucket_of_only_nulls_is_null_not_zero_and_not_absent() {
        let rows = [sample(at(10, 1), None, "")];
        let s = aggregate(
            &rows,
            &query(Aggregate::Sum, Bucket::Minute5, GapPolicy::Missing),
            NOW,
        );
        let p = s[0].points[0];
        assert_eq!(p.value, None);
        assert_eq!(p.state, PointState::NullOnly);
        assert_eq!(s[0].points.len(), 1);
    }

    #[test]
    fn the_range_is_half_open() {
        let rows = [
            sample(at(10, 0), Some(1.0), ""),
            sample(at(11, 0), Some(100.0), ""),
            sample(at(9, 59), Some(100.0), ""),
        ];
        let s = aggregate(
            &rows,
            &query(Aggregate::Sum, Bucket::Hour1, GapPolicy::Missing),
            NOW,
        );
        assert_eq!(values(&s[0]), [(at(10, 0), Some(1.0))]);
    }

    #[test]
    fn missing_leaves_absent_buckets_out() {
        let rows = [
            sample(at(10, 0), Some(1.0), ""),
            sample(at(10, 20), Some(2.0), ""),
        ];
        let s = aggregate(
            &rows,
            &query(Aggregate::Sum, Bucket::Minute5, GapPolicy::Missing),
            NOW,
        );
        assert_eq!(s[0].points.len(), 2);
    }

    #[test]
    fn zero_fill_adds_marked_zero_points_over_the_whole_range() {
        let rows = [sample(at(10, 0), Some(1.0), "")];
        let s = aggregate(
            &rows,
            &query(Aggregate::Sum, Bucket::Minute15, GapPolicy::Zero),
            NOW,
        );
        assert_eq!(s[0].points.len(), 4);
        assert_eq!(s[0].points[0].state, PointState::Observed);
        assert!(
            s[0].points[1..]
                .iter()
                .all(|p| p.state == PointState::Filled(Fill::Zero) && p.value == Some(0.0))
        );
    }

    #[test]
    fn carry_forward_repeats_only_after_the_first_observation() {
        let rows = [sample(at(10, 15), Some(7.0), "")];
        let s = aggregate(
            &rows,
            &query(Aggregate::Sum, Bucket::Minute15, GapPolicy::CarryForward),
            NOW,
        );
        assert_eq!(
            values(&s[0]),
            [
                (at(10, 15), Some(7.0)),
                (at(10, 30), Some(7.0)),
                (at(10, 45), Some(7.0))
            ]
        );
        assert_eq!(s[0].points[1].state, PointState::Filled(Fill::Carried));
    }

    #[test]
    fn linear_fill_joins_two_observations_and_never_extends_the_edges() {
        let rows = [
            sample(at(10, 15), Some(10.0), ""),
            sample(at(10, 45), Some(30.0), ""),
        ];
        let s = aggregate(
            &rows,
            &query(Aggregate::Sum, Bucket::Minute15, GapPolicy::Linear),
            NOW,
        );
        assert_eq!(
            values(&s[0]),
            [
                (at(10, 15), Some(10.0)),
                (at(10, 30), Some(20.0)),
                (at(10, 45), Some(30.0))
            ]
        );
        assert_eq!(s[0].points[1].state, PointState::Filled(Fill::Interpolated));
    }

    #[test]
    fn a_gap_policy_never_changes_an_observed_value() {
        let rows = [
            sample(at(10, 0), Some(5.0), ""),
            sample(at(10, 30), Some(9.0), ""),
        ];
        let base = aggregate(
            &rows,
            &query(Aggregate::Sum, Bucket::Minute15, GapPolicy::Missing),
            NOW,
        );
        for gap in [GapPolicy::Zero, GapPolicy::CarryForward, GapPolicy::Linear] {
            let s = aggregate(&rows, &query(Aggregate::Sum, Bucket::Minute15, gap), NOW);
            for p in &base[0].points {
                assert_eq!(s[0].point_at(p.start).map(|q| q.value), Some(p.value));
            }
        }
    }

    #[test]
    fn the_current_bucket_is_provisional() {
        let mut q = query(Aggregate::Sum, Bucket::Hour1, GapPolicy::Missing);
        q.span = TimeSpan::new(at(12, 0), at(14, 0));
        let rows = [
            sample(at(12, 5), Some(1.0), ""),
            sample(at(13, 30), Some(1.0), ""),
        ];
        let s = aggregate(&rows, &q, NOW);
        assert!(!s[0].points[0].provisional);
        assert!(s[0].points[1].provisional);
    }

    #[test]
    fn groups_make_separate_series_heaviest_first() {
        let rows = [
            sample(at(10, 1), Some(1.0), "a"),
            sample(at(10, 2), Some(50.0), "b"),
            sample(at(10, 3), Some(5.0), "c"),
        ];
        let s = aggregate(
            &rows,
            &query(Aggregate::Sum, Bucket::Hour1, GapPolicy::Missing),
            NOW,
        );
        let keys: Vec<&str> = s.iter().map(|x| x.key.as_str()).collect();
        assert_eq!(keys, ["b", "c", "a"]);
    }

    #[test]
    fn no_samples_make_no_series() {
        let s = aggregate(
            &[],
            &query(Aggregate::Sum, Bucket::Hour1, GapPolicy::Zero),
            NOW,
        );
        assert!(s.is_empty());
    }

    #[test]
    fn bucket_starts_floor_toward_the_past() {
        assert_eq!(bucket_start(at(10, 7), Bucket::Minute5), at(10, 5));
        assert_eq!(bucket_start(-1, Bucket::Minute1), -60);
    }
}
