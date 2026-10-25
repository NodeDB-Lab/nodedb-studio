//! Axes for the Monitoring charts. Plain Rust, so the choices are tested:
//! the chart only draws what these return. Time runs on a fixed window, so a
//! node that stopped reporting draws a line that ends early, never one that
//! reaches the right edge.

use crate::models::cluster_monitor::{Metric, Sample};
use crate::models::format::clock;
use crate::models::timeseries_axis::value_ticks;

/// Tick spacings, in seconds, from fine to coarse.
const STEPS: [i64; 11] = [
    60, 120, 300, 600, 900, 1_800, 3_600, 7_200, 10_800, 21_600, 43_200,
];

/// Time ticks on round clock times inside `start..=end`, at most `target`.
pub fn time_ticks(start: i64, end: i64, target: usize) -> Vec<(i64, String)> {
    let span = (end - start).max(1);
    let step = STEPS
        .iter()
        .copied()
        .find(|s| span / s <= target.max(1) as i64)
        .unwrap_or(86_400);
    let first = start.div_euclid(step) * step + if start.rem_euclid(step) == 0 { 0 } else { step };
    (0..)
        .map(|i| first + i * step)
        .take_while(|t| *t <= end)
        .map(|t| (t, clock(t, 0)))
        .collect()
}

/// The value axis of a metric whose samples top out at `max`. CPU always
/// spans 0 to 100 percent. Every axis starts at zero.
pub fn value_axis(metric: Metric, max: f64) -> Vec<f64> {
    match metric {
        Metric::Cpu => vec![0.0, 25.0, 50.0, 75.0, 100.0],
        Metric::Queries | Metric::P99Latency | Metric::Memory => value_ticks(max, 4),
    }
}

/// The index of the sample nearest in time to `t`.
pub fn nearest(samples: &[Sample], t: i64) -> Option<usize> {
    samples
        .iter()
        .enumerate()
        .min_by_key(|(_, s)| (s.at - t).abs())
        .map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::format::epoch;

    #[test]
    fn an_hour_ticks_every_ten_minutes_on_the_clock() {
        let end = epoch(2026, 6, 13, 13, 40, 0);
        let ticks = time_ticks(end - 3_600, end, 6);
        let labels: Vec<_> = ticks.iter().map(|(_, l)| l.as_str()).collect();
        assert_eq!(
            labels,
            [
                "12:40", "12:50", "13:00", "13:10", "13:20", "13:30", "13:40"
            ]
        );
    }

    #[test]
    fn a_day_ticks_on_whole_hours() {
        let end = epoch(2026, 6, 13, 13, 40, 0);
        let ticks = time_ticks(end - 86_400, end, 6);
        assert!(ticks.len() <= 7);
        assert!(ticks.iter().all(|(t, _)| t % 3_600 == 0));
    }

    #[test]
    fn ticks_stay_inside_the_window() {
        let end = epoch(2026, 6, 13, 13, 41, 30);
        for (t, _) in time_ticks(end - 900, end, 5) {
            assert!(t >= end - 900 && t <= end);
        }
    }

    #[test]
    fn cpu_spans_zero_to_a_hundred() {
        assert_eq!(value_axis(Metric::Cpu, 78.0).last(), Some(&100.0));
        let q = value_axis(Metric::Queries, 1_892.0);
        assert_eq!(q.first(), Some(&0.0));
        assert!(q.last().is_some_and(|t| *t >= 1_892.0));
    }

    #[test]
    fn nearest_finds_the_closest_sample() {
        let s: Vec<Sample> = [0, 60, 120]
            .iter()
            .map(|t| Sample { at: *t, value: 1.0 })
            .collect();
        assert_eq!(nearest(&s, 70), Some(1));
        assert_eq!(nearest(&s, 1_000), Some(2));
        assert_eq!(nearest(&[], 5), None);
    }
}
