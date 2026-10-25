//! Server telemetry: what each Origin node reports about itself over time.
//! It is telemetry from the servers, not collection data. Every node row
//! carries the time it was last observed, and a node that stopped reporting
//! stays stale with that time. A stale value never reads as current.

use serde::{Deserialize, Serialize};

use crate::models::cluster_place::GIB;

/// A node whose last report is older than this is stale.
pub const STALE_AFTER_SECS: i64 = 90;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Metric {
    Queries,
    P99Latency,
    Cpu,
    Memory,
}

impl Metric {
    pub const ALL: [Metric; 4] = [
        Metric::Queries,
        Metric::P99Latency,
        Metric::Cpu,
        Metric::Memory,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Metric::Queries => "Queries",
            Metric::P99Latency => "p99 latency",
            Metric::Cpu => "CPU usage",
            Metric::Memory => "Memory usage",
        }
    }

    pub fn unit(self) -> &'static str {
        match self {
            Metric::Queries => "req/s",
            Metric::P99Latency => "ms",
            Metric::Cpu => "%",
            Metric::Memory => "GiB",
        }
    }

    /// Decimal places of a summary value.
    pub fn decimals(self) -> usize {
        match self {
            Metric::Queries | Metric::Cpu => 0,
            Metric::P99Latency | Metric::Memory => 1,
        }
    }

    /// The panel id of the chart.
    pub fn panel(self) -> &'static str {
        match self {
            Metric::Queries => "queries",
            Metric::P99Latency => "latency",
            Metric::Cpu => "cpu",
            Metric::Memory => "memory",
        }
    }

    /// The `--viz-N` color index. One color per measure, never reused.
    pub fn color(self) -> usize {
        match self {
            Metric::Queries => 0,
            Metric::P99Latency => 3,
            Metric::Cpu => 1,
            Metric::Memory => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TelemetryRange {
    Minutes15,
    #[default]
    Hour1,
    Hours6,
    Hours24,
}

impl TelemetryRange {
    pub const ALL: [TelemetryRange; 4] = [
        TelemetryRange::Minutes15,
        TelemetryRange::Hour1,
        TelemetryRange::Hours6,
        TelemetryRange::Hours24,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TelemetryRange::Minutes15 => "Last 15 minutes",
            TelemetryRange::Hour1 => "Last 1 hour",
            TelemetryRange::Hours6 => "Last 6 hours",
            TelemetryRange::Hours24 => "Last 24 hours",
        }
    }

    pub fn seconds(self) -> i64 {
        match self {
            TelemetryRange::Minutes15 => 900,
            TelemetryRange::Hour1 => 3_600,
            TelemetryRange::Hours6 => 21_600,
            TelemetryRange::Hours24 => 86_400,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    pub at: i64,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricSeries {
    pub metric: Metric,
    pub samples: Vec<Sample>,
}

impl MetricSeries {
    pub fn average(&self) -> Option<f64> {
        if self.samples.is_empty() {
            return None;
        }
        Some(self.samples.iter().map(|s| s.value).sum::<f64>() / self.samples.len() as f64)
    }

    pub fn max(&self) -> Option<f64> {
        self.samples.iter().map(|s| s.value).reduce(f64::max)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    Ready,
    Stale,
}

impl Freshness {
    pub fn label(self) -> &'static str {
        match self {
            Freshness::Ready => "Ready",
            Freshness::Stale => "Stale",
        }
    }
}

/// The latest report of one node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeTelemetry {
    pub node_id: String,
    pub node: String,
    pub cpu_percent: f64,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub disk_used_bytes: u64,
    pub disk_total_bytes: u64,
    pub net_in_mbs: f64,
    pub net_out_mbs: f64,
    /// When the node last reported, in seconds since the epoch.
    pub observed_at: i64,
}

impl NodeTelemetry {
    /// Stale when the report is older than `STALE_AFTER_SECS` at `now`.
    pub fn freshness(&self, now: i64) -> Freshness {
        if now - self.observed_at > STALE_AFTER_SECS {
            Freshness::Stale
        } else {
            Freshness::Ready
        }
    }

    pub fn memory_percent(&self) -> u64 {
        (self.memory_used_bytes * 100)
            .checked_div(self.memory_total_bytes)
            .unwrap_or(0)
    }

    pub fn disk_percent(&self) -> u64 {
        (self.disk_used_bytes * 100)
            .checked_div(self.disk_total_bytes)
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Telemetry {
    /// The server's clock when it answered.
    pub observed_at: i64,
    pub range: TelemetryRange,
    /// The node the charts show: `None` is every node together.
    pub scope: Option<String>,
    pub series: Vec<MetricSeries>,
    pub nodes: Vec<NodeTelemetry>,
}

impl Telemetry {
    pub fn series_of(&self, metric: Metric) -> Option<&MetricSeries> {
        self.series.iter().find(|s| s.metric == metric)
    }
}

/// `20.4 GiB` or `1.0 TiB`: a capacity in the unit that reads best, with one
/// decimal place so the table columns line up.
pub fn capacity_text(bytes: u64) -> String {
    let gib = bytes as f64 / GIB as f64;
    if gib >= 1024.0 {
        format!("{:.1} TiB", gib / 1024.0)
    } else if gib < 100.0 {
        format!("{gib:.1} GiB")
    } else {
        format!("{} GiB", gib.round() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(observed_at: i64) -> NodeTelemetry {
        NodeTelemetry {
            node_id: "nd_01".into(),
            node: "node-1".into(),
            cpu_percent: 58.0,
            memory_used_bytes: 20 * GIB,
            memory_total_bytes: 32 * GIB,
            disk_used_bytes: 420 * GIB,
            disk_total_bytes: 1024 * GIB,
            net_in_mbs: 312.0,
            net_out_mbs: 286.0,
            observed_at,
        }
    }

    #[test]
    fn an_old_report_is_stale_never_ready() {
        assert_eq!(node(1_000).freshness(1_030), Freshness::Ready);
        assert_eq!(
            node(1_000).freshness(1_000 + STALE_AFTER_SECS),
            Freshness::Ready
        );
        assert_eq!(
            node(1_000).freshness(1_000 + STALE_AFTER_SECS + 1),
            Freshness::Stale
        );
    }

    #[test]
    fn percentages_are_whole() {
        let n = node(0);
        assert_eq!(n.memory_percent(), 62);
        assert_eq!(n.disk_percent(), 41);
    }

    #[test]
    fn capacity_picks_a_unit() {
        assert_eq!(capacity_text(20 * GIB + GIB / 2), "20.5 GiB");
        assert_eq!(capacity_text(420 * GIB), "420 GiB");
        assert_eq!(capacity_text(1024 * GIB), "1.0 TiB");
    }

    #[test]
    fn summary_reads_the_samples() {
        let s = MetricSeries {
            metric: Metric::Cpu,
            samples: vec![Sample { at: 0, value: 10.0 }, Sample { at: 1, value: 30.0 }],
        };
        assert_eq!(s.average(), Some(20.0));
        assert_eq!(s.max(), Some(30.0));
        let empty = MetricSeries {
            metric: Metric::Cpu,
            samples: Vec::new(),
        };
        assert_eq!(empty.average(), None);
    }
}
