//! Node telemetry fixtures. Series are generated from fixed parameters, so
//! a read always returns the same samples. node-5 stopped reporting at
//! 13:36:02: its series end there and its row stays stale with that time.

use crate::data::mock::cluster::members::{NOW, node_id, node_name};
use crate::models::cluster_monitor::{
    Metric, MetricSeries, NodeTelemetry, Sample, Telemetry, TelemetryRange,
};
use crate::models::cluster_place::GIB;
use crate::models::format::epoch;

/// What a node reports last, and the level its series hover around.
struct Profile {
    n: u32,
    queries: f64,
    p99: f64,
    cpu: f64,
    memory: f64,
    last_cpu: f64,
    memory_used_gib: f64,
    disk_gib: u64,
    net: (f64, f64),
    observed_at: i64,
}

fn profiles() -> [Profile; 5] {
    let live = NOW;
    [
        Profile {
            n: 1,
            queries: 1230.0,
            p99: 12.4,
            cpu: 52.0,
            memory: 18.6,
            last_cpu: 58.0,
            memory_used_gib: 20.4,
            disk_gib: 420,
            net: (312.0, 286.0),
            observed_at: live,
        },
        Profile {
            n: 2,
            queries: 1080.0,
            p99: 11.0,
            cpu: 46.0,
            memory: 17.1,
            last_cpu: 46.0,
            memory_used_gib: 17.1,
            disk_gib: 390,
            net: (268.0, 241.0),
            observed_at: live,
        },
        Profile {
            n: 3,
            queries: 1150.0,
            p99: 13.0,
            cpu: 44.0,
            memory: 15.0,
            last_cpu: 44.0,
            memory_used_gib: 15.2,
            disk_gib: 612,
            net: (284.0, 270.0),
            observed_at: live,
        },
        Profile {
            n: 4,
            queries: 700.0,
            p99: 15.0,
            cpu: 38.0,
            memory: 12.0,
            last_cpu: 41.0,
            memory_used_gib: 12.4,
            disk_gib: 140,
            net: (190.0, 176.0),
            observed_at: live,
        },
        Profile {
            n: 5,
            queries: 600.0,
            p99: 14.0,
            cpu: 12.0,
            memory: 6.3,
            last_cpu: 12.0,
            memory_used_gib: 6.3,
            disk_gib: 288,
            net: (48.0, 62.0),
            observed_at: epoch(2026, 6, 13, 13, 36, 2),
        },
    ]
}

/// A repeatable value in `-1..1` for `(seed, i)`.
fn noise(seed: u64, i: u64) -> f64 {
    let mut z = seed
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(i.wrapping_mul(0xBF58_476D_1CE4_E5B9));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 52) as f64 - 1.0
}

/// The sample times of a range, oldest first: 61 points ending at `NOW`.
fn times(range: TelemetryRange) -> Vec<i64> {
    let step = range.seconds() / 60;
    (0..=60).map(|i| NOW - range.seconds() + i * step).collect()
}

fn value(p: &Profile, metric: Metric, i: usize, count: usize) -> f64 {
    let seed = u64::from(p.n) * 17
        + match metric {
            Metric::Queries => 1,
            Metric::P99Latency => 2,
            Metric::Cpu => 3,
            Metric::Memory => 4,
        };
    let wave = (i as f64 / count as f64 * std::f64::consts::TAU * 1.5).sin();
    let jitter = noise(seed, i as u64);
    match metric {
        Metric::Queries => (p.queries * (1.0 + 0.16 * wave + 0.07 * jitter)).max(0.0),
        Metric::P99Latency => {
            // Two short spikes ride on the base.
            let spike = if i == count * 3 / 10 {
                2.8
            } else if i == count * 7 / 10 {
                1.1
            } else {
                0.0
            };
            (p.p99 * (1.0 + 0.12 * jitter + spike)).max(0.5)
        }
        Metric::Cpu => (p.cpu * (1.0 + 0.14 * wave + 0.08 * jitter)).clamp(1.0, 100.0),
        Metric::Memory => p.memory * (1.0 + 0.06 * wave + 0.02 * jitter),
    }
}

/// How `nodes` combine into one cluster-wide series.
fn combine(metric: Metric, values: &[f64]) -> f64 {
    let sum: f64 = values.iter().sum();
    match metric {
        Metric::Queries | Metric::Memory => sum,
        Metric::P99Latency => values.iter().copied().fold(0.0, f64::max),
        Metric::Cpu => sum / values.len().max(1) as f64,
    }
}

fn series(selected: &[&Profile], metric: Metric, range: TelemetryRange) -> MetricSeries {
    let at = times(range);
    let count = at.len();
    let samples = at
        .iter()
        .enumerate()
        .filter_map(|(i, t)| {
            let values: Vec<f64> = selected
                .iter()
                .filter(|p| *t <= p.observed_at)
                .map(|p| value(p, metric, i, count))
                .collect();
            if values.is_empty() {
                None
            } else {
                Some(Sample {
                    at: *t,
                    value: combine(metric, &values),
                })
            }
        })
        .collect();
    MetricSeries { metric, samples }
}

/// Telemetry for `node` (a node id), or every node together when `None`. An
/// unknown id selects no node, so its charts are empty.
pub fn telemetry(node: Option<&str>, range: TelemetryRange) -> Telemetry {
    let all = profiles();
    let selected: Vec<&Profile> = all
        .iter()
        .filter(|p| node.is_none_or(|id| node_id(p.n) == id))
        .collect();
    let nodes = all
        .iter()
        .map(|p| NodeTelemetry {
            node_id: node_id(p.n),
            node: node_name(p.n),
            cpu_percent: p.last_cpu,
            memory_used_bytes: (p.memory_used_gib * GIB as f64) as u64,
            memory_total_bytes: 32 * GIB,
            disk_used_bytes: p.disk_gib * GIB,
            disk_total_bytes: 1024 * GIB,
            net_in_mbs: p.net.0,
            net_out_mbs: p.net.1,
            observed_at: p.observed_at,
        })
        .collect();
    Telemetry {
        observed_at: NOW,
        range,
        scope: node.map(str::to_string),
        series: Metric::ALL
            .iter()
            .map(|m| series(&selected, *m, range))
            .collect(),
        nodes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_read_repeats() {
        let a = telemetry(Some("nd_01"), TelemetryRange::Hour1);
        let b = telemetry(Some("nd_01"), TelemetryRange::Hour1);
        assert_eq!(a, b);
    }

    #[test]
    fn every_range_has_sixty_one_samples_ending_now() {
        for r in TelemetryRange::ALL {
            let t = telemetry(Some("nd_01"), r);
            for s in &t.series {
                assert_eq!(s.samples.len(), 61);
                assert_eq!(s.samples.last().map(|x| x.at), Some(NOW));
            }
        }
    }

    #[test]
    fn a_stale_node_stops_at_its_last_observation() {
        let t = telemetry(Some("nd_05"), TelemetryRange::Hour1);
        let last = t.series[0].samples.last().map(|s| s.at).unwrap_or(0);
        assert!(last <= epoch(2026, 6, 13, 13, 36, 2));
        assert!(last < NOW);
    }

    #[test]
    fn cluster_latency_is_the_worst_node_and_queries_add_up() {
        let all = telemetry(None, TelemetryRange::Hour1);
        let one = telemetry(Some("nd_01"), TelemetryRange::Hour1);
        let q_all = all.series_of(Metric::Queries).map(|s| s.samples[5].value);
        let q_one = one.series_of(Metric::Queries).map(|s| s.samples[5].value);
        assert!(q_all > q_one);
        let l_all = all
            .series_of(Metric::P99Latency)
            .map(|s| s.samples[5].value);
        let l_one = one
            .series_of(Metric::P99Latency)
            .map(|s| s.samples[5].value);
        assert!(l_all >= l_one);
    }

    #[test]
    fn an_unknown_node_selects_no_samples() {
        let t = telemetry(Some("nd_99"), TelemetryRange::Hour1);
        assert!(t.series.iter().all(|s| s.samples.is_empty()));
        assert_eq!(t.nodes.len(), 5);
    }
}
