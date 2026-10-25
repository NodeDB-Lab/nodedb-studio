//! Timeseries answer: points, series, and the one result chart, table, and
//! export all read.

use crate::models::format::date_second;
use crate::models::timeseries::ExploreQuery;

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
    use crate::models::format::epoch;
    use crate::models::records::Filter;
    use crate::models::timeseries::{Aggregate, Bucket, GapPolicy, Measure, TimeSpan};

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
}
