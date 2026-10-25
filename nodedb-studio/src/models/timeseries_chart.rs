//! Chart geometry: where a bucket, a value, and a marker go. Plain Rust, so
//! the places are tested. The component draws what these return.
//!
//! Buckets sit on a band scale: bucket `i` owns the `i`th equal slice of the
//! plot width and its mark is centered in it. Values sit on a linear scale
//! from zero to the top tick.

use crate::models::timeseries::{ExploreResult, Series};
use crate::models::timeseries_axis::{label_stride, spans_days, value_ticks, x_label};

/// The drawing box and its margins, in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub width: f64,
    pub height: f64,
    pub left: f64,
    pub right: f64,
    pub top: f64,
    pub bottom: f64,
}

impl Frame {
    pub fn plot_width(&self) -> f64 {
        (self.width - self.left - self.right).max(1.0)
    }

    pub fn plot_height(&self) -> f64 {
        (self.height - self.top - self.bottom).max(1.0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Geometry {
    pub frame: Frame,
    /// Value ticks from zero to the top of the axis.
    pub ticks: Vec<f64>,
    /// The value at the top of the plot.
    pub top_value: f64,
    pub buckets: usize,
    /// Width of one bucket's slice.
    pub band: f64,
    /// Bucket indexes that carry a label, with the label.
    pub x_labels: Vec<(usize, String)>,
}

/// Pixels a label needs: about seven per character of 12px text, plus air.
fn label_width(text: &str) -> f64 {
    text.chars().count() as f64 * 7.2 + 14.0
}

impl Geometry {
    /// Lay out `result`'s buckets and the visible series' values in `frame`.
    pub fn new(frame: Frame, result: &ExploreResult, visible: &[&Series], y_ticks: usize) -> Self {
        let n = result.buckets.len();
        let max = visible
            .iter()
            .flat_map(|s| s.points.iter().filter_map(|p| p.value))
            .fold(0.0_f64, f64::max);
        let ticks = value_ticks(max, y_ticks);
        let top_value = ticks.last().copied().unwrap_or(1.0);
        let band = frame.plot_width() / n.max(1) as f64;
        let days = spans_days(&result.buckets);
        let labels: Vec<String> = result
            .buckets
            .iter()
            .map(|b| x_label(*b, result.query.bucket, days))
            .collect();
        let widest = labels
            .iter()
            .map(|l| label_width(l))
            .fold(0.0_f64, f64::max);
        let fit = (frame.plot_width() / widest.max(1.0)).floor().max(1.0) as usize;
        let stride = label_stride(n, fit);
        let x_labels = labels
            .into_iter()
            .enumerate()
            .filter(|(i, _)| i % stride == 0)
            .collect();
        Self {
            frame,
            ticks,
            top_value,
            buckets: n,
            band,
            x_labels,
        }
    }

    /// The center of bucket `i`.
    pub fn x(&self, i: usize) -> f64 {
        self.frame.left + self.band * (i as f64 + 0.5)
    }

    /// The pixel row of `value`, clipped to the plot.
    pub fn y(&self, value: f64) -> f64 {
        let f = (value / self.top_value).clamp(0.0, 1.0);
        self.frame.top + self.frame.plot_height() * (1.0 - f)
    }

    pub fn baseline(&self) -> f64 {
        self.frame.top + self.frame.plot_height()
    }

    /// What a pointer handler needs to find the bucket under the pointer.
    pub fn hit_map(&self) -> HitMap {
        HitMap {
            band: self.band,
            buckets: self.buckets,
        }
    }
}

/// The part of the geometry a pointer handler keeps: it is `Copy`, so every
/// handler owns one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitMap {
    band: f64,
    buckets: usize,
}

impl HitMap {
    pub fn bucket_at(self, x: f64) -> Option<usize> {
        if self.buckets == 0 || x < 0.0 || x >= self.band * self.buckets as f64 {
            return None;
        }
        Some(((x / self.band) as usize).min(self.buckets - 1))
    }
}

/// Runs of consecutive buckets that hold a value: a line joins the points of
/// one run and breaks between runs, so a gap is drawn as a gap. Each entry is
/// a bucket index and its value.
pub fn runs(series: &Series, buckets: &[i64]) -> Vec<Vec<(usize, f64)>> {
    let mut out: Vec<Vec<(usize, f64)>> = Vec::new();
    let mut open = false;
    for (i, start) in buckets.iter().enumerate() {
        match series.point_at(*start).and_then(|p| p.value) {
            Some(v) => {
                if !open {
                    out.push(Vec::new());
                    open = true;
                }
                if let Some(run) = out.last_mut() {
                    run.push((i, v));
                }
            }
            None => open = false,
        }
    }
    out
}

/// The path of marker `shape` (0 to 5) of radius `r` at `(cx, cy)`. Shapes
/// differ, so series stay apart without their color.
pub fn marker(shape: usize, cx: f64, cy: f64, r: f64) -> String {
    match shape % 6 {
        0 => format!(
            "M{:.1} {cy:.1}a{r:.1} {r:.1} 0 1 0 {:.1} 0a{r:.1} {r:.1} 0 1 0 {:.1} 0z",
            cx - r,
            2.0 * r,
            -2.0 * r
        ),
        1 => format!(
            "M{:.1} {:.1}h{:.1}v{:.1}h{:.1}z",
            cx - r,
            cy - r,
            2.0 * r,
            2.0 * r,
            -2.0 * r
        ),
        2 => format!(
            "M{cx:.1} {:.1}L{:.1} {cy:.1}L{cx:.1} {:.1}L{:.1} {cy:.1}z",
            cy - r * 1.25,
            cx + r * 1.25,
            cy + r * 1.25,
            cx - r * 1.25
        ),
        3 => format!(
            "M{cx:.1} {:.1}L{:.1} {:.1}L{:.1} {:.1}z",
            cy - r * 1.2,
            cx + r * 1.15,
            cy + r * 0.95,
            cx - r * 1.15,
            cy + r * 0.95
        ),
        4 => format!(
            "M{:.1} {:.1}h{:.1}v{:.1}h{:.1}v{:.1}h{:.1}v{:.1}h{:.1}v{:.1}h{:.1}v{:.1}z",
            cx - r * 0.4,
            cy - r,
            r * 0.8,
            r * 0.6,
            r * 0.6,
            r * 0.8,
            -r * 0.6,
            r * 0.6,
            -r * 0.8,
            -r * 0.6,
            -r * 0.6,
            -r * 0.8
        ),
        _ => format!(
            "M{:.1} {:.1}L{:.1} {:.1}L{:.1} {:.1}L{:.1} {:.1}L{:.1} {:.1}L{:.1} {:.1}z",
            cx - r,
            cy,
            cx - r * 0.5,
            cy - r * 0.87,
            cx + r * 0.5,
            cy - r * 0.87,
            cx + r,
            cy,
            cx + r * 0.5,
            cy + r * 0.87,
            cx - r * 0.5,
            cy + r * 0.87
        ),
    }
}

/// The value axis title: `Revenue (USD)`, `Rows`, `Average quantity`.
pub fn y_title(result: &ExploreResult) -> String {
    use crate::models::timeseries::Aggregate;
    let q = &result.query;
    let Some(field) = &q.measure.field else {
        return "Rows".to_string();
    };
    let mut chars = field.chars();
    let name = chars
        .next()
        .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
        .unwrap_or_default();
    let base = match q.measure.aggregate {
        Aggregate::Sum | Aggregate::Count => name,
        Aggregate::Avg => format!("Average {}", field),
        Aggregate::Min => format!("Minimum {}", field),
        Aggregate::Max => format!("Maximum {}", field),
    };
    match &result.unit {
        Some(u) => format!("{base} ({u})"),
        None => base,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::format::epoch;
    use crate::models::timeseries::{
        Aggregate, Bucket, ExploreQuery, Fill, GapPolicy, Measure, Point, PointState, TimeSpan,
    };

    fn frame() -> Frame {
        Frame {
            width: 700.0,
            height: 340.0,
            left: 60.0,
            right: 20.0,
            top: 10.0,
            bottom: 40.0,
        }
    }

    fn result(buckets: usize, points: &[(usize, Option<f64>)]) -> ExploreResult {
        let start = epoch(2026, 6, 13, 10, 0, 0);
        let starts: Vec<i64> = (0..buckets as i64).map(|i| start + i * 300).collect();
        ExploreResult {
            query: ExploreQuery {
                collection: "sales".into(),
                time_field: "sold_at".into(),
                measure: Measure::of(Aggregate::Sum, "revenue"),
                filters: Vec::new(),
                bucket: Bucket::Minute5,
                group_by: None,
                gap: GapPolicy::Missing,
                span: TimeSpan::new(start, start + buckets as i64 * 300),
            },
            observed_at: start,
            series: vec![Series {
                key: String::new(),
                weight: 0.0,
                points: points
                    .iter()
                    .map(|(i, v)| Point {
                        start: starts[*i],
                        value: *v,
                        state: PointState::Observed,
                        provisional: false,
                    })
                    .collect(),
            }],
            buckets: starts,
            rows_scanned: 0,
            rows_matched: 0,
            unit: Some("USD".into()),
        }
    }

    #[test]
    fn twelve_buckets_get_twelve_slices_and_centered_marks() {
        let r = result(12, &[(0, Some(1800.0))]);
        let vis: Vec<&Series> = r.series.iter().collect();
        let g = Geometry::new(frame(), &r, &vis, 4);
        assert_eq!(g.buckets, 12);
        assert!((g.band - 620.0 / 12.0).abs() < 1e-9);
        assert!((g.x(0) - (60.0 + g.band / 2.0)).abs() < 1e-9);
        assert!((g.x(11) - (60.0 + g.band * 11.5)).abs() < 1e-9);
    }

    #[test]
    fn values_sit_between_the_baseline_and_the_top_tick() {
        let r = result(12, &[(0, Some(1800.0)), (1, Some(2950.0))]);
        let vis: Vec<&Series> = r.series.iter().collect();
        let g = Geometry::new(frame(), &r, &vis, 4);
        assert!(g.top_value >= 2950.0);
        assert_eq!(g.y(0.0), g.baseline());
        assert_eq!(g.y(g.top_value), 10.0);
        assert!(g.y(1800.0) < g.baseline() && g.y(1800.0) > 10.0);
        assert_eq!(g.y(g.top_value * 5.0), 10.0, "clipped to the plot");
    }

    #[test]
    fn a_pixel_maps_back_to_its_bucket() {
        let r = result(12, &[(0, Some(1.0))]);
        let vis: Vec<&Series> = r.series.iter().collect();
        let g = Geometry::new(frame(), &r, &vis, 4);
        assert_eq!(g.hit_map().bucket_at(0.0), Some(0));
        assert_eq!(g.hit_map().bucket_at(g.band * 3.5), Some(3));
        assert_eq!(g.hit_map().bucket_at(g.band * 12.0 - 0.01), Some(11));
        assert_eq!(g.hit_map().bucket_at(-1.0), None);
        assert_eq!(g.hit_map().bucket_at(g.band * 12.0 + 1.0), None);
    }

    #[test]
    fn x_labels_never_outnumber_what_fits() {
        let r = result(288, &[(0, Some(1.0))]);
        let vis: Vec<&Series> = r.series.iter().collect();
        let g = Geometry::new(frame(), &r, &vis, 4);
        let widest = g
            .x_labels
            .iter()
            .map(|(_, l)| label_width(l))
            .fold(0.0, f64::max);
        assert!(g.x_labels.len() as f64 * widest <= g.frame.plot_width() + widest);
        assert_eq!(g.x_labels[0].0, 0, "the first bucket is always labelled");
    }

    /// The design's twelve five-minute buckets all carry a label.
    #[test]
    fn twelve_buckets_are_all_labelled_at_a_wide_chart() {
        let r = result(12, &[(0, Some(1.0))]);
        let vis: Vec<&Series> = r.series.iter().collect();
        let g = Geometry::new(frame(), &r, &vis, 4);
        assert_eq!(g.x_labels.len(), 12);
        assert_eq!(g.x_labels[0].1, "10:00");
        assert_eq!(g.x_labels[11].1, "10:55");
    }

    #[test]
    fn a_gap_breaks_the_line_into_two_runs() {
        let r = result(
            12,
            &[
                (0, Some(1.0)),
                (1, Some(2.0)),
                (3, Some(5.0)),
                (4, Some(6.0)),
            ],
        );
        let runs = runs(&r.series[0], &r.buckets);
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0], [(0, 1.0), (1, 2.0)]);
        assert_eq!(runs[1], [(3, 5.0), (4, 6.0)]);
    }

    #[test]
    fn a_null_bucket_breaks_the_line_too() {
        let r = result(5, &[(0, Some(1.0)), (1, None), (2, Some(3.0))]);
        assert_eq!(runs(&r.series[0], &r.buckets).len(), 2);
    }

    #[test]
    fn filled_points_join_the_line() {
        let mut r = result(4, &[(0, Some(1.0)), (2, Some(3.0))]);
        r.series[0].points.push(Point {
            start: r.buckets[1],
            value: Some(2.0),
            state: PointState::Filled(Fill::Interpolated),
            provisional: false,
        });
        assert_eq!(runs(&r.series[0], &r.buckets).len(), 1);
    }

    #[test]
    fn the_axis_title_names_the_measure_and_unit() {
        let r = result(2, &[]);
        assert_eq!(y_title(&r), "Revenue (USD)");
        let mut c = r.clone();
        c.query.measure = Measure::count_rows();
        c.unit = None;
        assert_eq!(y_title(&c), "Rows");
    }

    #[test]
    fn every_marker_shape_is_a_closed_path() {
        for i in 0..6 {
            let d = marker(i, 10.0, 10.0, 4.0);
            assert!(d.starts_with('M') && d.ends_with('z'), "{d}");
        }
        assert_ne!(marker(0, 1.0, 1.0, 3.0), marker(1, 1.0, 1.0, 3.0));
    }
}
