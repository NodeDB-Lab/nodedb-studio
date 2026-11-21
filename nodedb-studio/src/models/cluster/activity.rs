//! Cluster activity: membership changes, leadership changes, and the history
//! of administrative operations. Each event names the node that reported it
//! and, when it has one, the group and the operation it belongs to. Nothing
//! here states a cause the server did not report.

use serde::{Deserialize, Serialize};

use crate::models::feeds::Tone;
use crate::models::format::{civil, clock_second, same_day};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl Severity {
    pub const ALL: [Severity; 3] = [Severity::Info, Severity::Warning, Severity::Error];

    pub fn label(self) -> &'static str {
        match self {
            Severity::Info => "Info",
            Severity::Warning => "Warning",
            Severity::Error => "Error",
        }
    }

    pub fn tone(self) -> Tone {
        match self {
            Severity::Info => Tone::Ok,
            Severity::Warning => Tone::Warn,
            Severity::Error => Tone::Err,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventKind {
    LeaderElected,
    LeadershipTransferred,
    HeartbeatDelayed,
    VoterUnreachable,
    NodeConnected,
    NodeUnreachable,
    SnapshotCreated,
    SnapshotInstalled,
    PlacementJobStarted,
    PlacementJobCompleted,
    ConfigChangeApplied,
}

impl EventKind {
    pub fn label(self) -> &'static str {
        match self {
            EventKind::LeaderElected => "Leader elected",
            EventKind::LeadershipTransferred => "Leadership transferred",
            EventKind::HeartbeatDelayed => "Heartbeat delayed",
            EventKind::VoterUnreachable => "Voter unreachable",
            EventKind::NodeConnected => "Node connected",
            EventKind::NodeUnreachable => "Node unreachable",
            EventKind::SnapshotCreated => "Snapshot created",
            EventKind::SnapshotInstalled => "Snapshot installed",
            EventKind::PlacementJobStarted => "Placement job started",
            EventKind::PlacementJobCompleted => "Placement job completed",
            EventKind::ConfigChangeApplied => "Config change applied",
        }
    }

    /// The scope sentence the details panel prints under the event.
    pub fn scope_note(self) -> &'static str {
        match self {
            EventKind::LeaderElected | EventKind::LeadershipTransferred => {
                "Leadership applies to this Raft group."
            }
            EventKind::HeartbeatDelayed | EventKind::VoterUnreachable => {
                "Heartbeats are observed by this group's leader."
            }
            EventKind::NodeConnected | EventKind::NodeUnreachable => {
                "Reachability belongs to the node. It says nothing about any group's quorum."
            }
            EventKind::SnapshotCreated | EventKind::SnapshotInstalled => {
                "Snapshots belong to this Raft group."
            }
            EventKind::PlacementJobStarted | EventKind::PlacementJobCompleted => {
                "A replica move changes this Raft group's membership."
            }
            EventKind::ConfigChangeApplied => "A configuration change applies to this Raft group.",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterEvent {
    /// Stable event id, e.g. `evt_98142`.
    pub id: String,
    /// When Studio's server observed it, in seconds since the epoch.
    pub observed_at: i64,
    /// When the source node says it happened.
    pub event_time: i64,
    pub severity: Severity,
    pub kind: EventKind,
    pub group: Option<String>,
    pub node_id: String,
    pub node: String,
    pub term: Option<u64>,
    pub previous_leader: Option<String>,
    pub leader: Option<String>,
    pub voters: Vec<String>,
    /// The node that reported the event.
    pub source: String,
    /// The operation the event belongs to, e.g. `op_72`.
    pub correlation: Option<String>,
    /// Who requested it, for an administrative operation.
    pub actor: Option<String>,
}

impl ClusterEvent {
    /// Whether `needle` (lowercase) appears in a field the table shows.
    pub fn matches(&self, needle: &str) -> bool {
        if needle.is_empty() {
            return true;
        }
        let fields = [
            Some(self.id.as_str()),
            Some(self.kind.label()),
            self.group.as_deref(),
            Some(self.node.as_str()),
            self.correlation.as_deref(),
        ];
        fields
            .into_iter()
            .flatten()
            .any(|f| f.to_lowercase().contains(needle))
    }
}

/// `13:35:41` on the day of `reference`, else `06-12 09:41:03`. The column
/// header says UTC.
pub fn stamp(at: i64, reference: i64) -> String {
    if same_day(at, reference, 0) {
        clock_second(at, 0)
    } else {
        let c = civil(at, 0);
        format!("{:02}-{:02} {}", c.month, c.day, clock_second(at, 0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActivityRange {
    Hour1,
    #[default]
    Hours24,
    Days7,
}

impl ActivityRange {
    pub const ALL: [ActivityRange; 3] = [
        ActivityRange::Hour1,
        ActivityRange::Hours24,
        ActivityRange::Days7,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ActivityRange::Hour1 => "Last hour",
            ActivityRange::Hours24 => "Last 24 hours",
            ActivityRange::Days7 => "Last 7 days",
        }
    }

    pub fn seconds(self) -> i64 {
        match self {
            ActivityRange::Hour1 => 3_600,
            ActivityRange::Hours24 => 86_400,
            ActivityRange::Days7 => 7 * 86_400,
        }
    }
}

/// What the server filters. The search text filters in Studio.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ActivityQuery {
    pub range: ActivityRange,
    pub severity: Option<Severity>,
    /// A node id.
    pub node: Option<String>,
    pub group: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivityLog {
    pub observed_at: i64,
    /// Newest first.
    pub events: Vec<ClusterEvent>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event() -> ClusterEvent {
        ClusterEvent {
            id: "evt_1".into(),
            observed_at: 10,
            event_time: 9,
            severity: Severity::Info,
            kind: EventKind::LeaderElected,
            group: Some("data-12".into()),
            node_id: "nd_02".into(),
            node: "node-2".into(),
            term: Some(91),
            previous_leader: None,
            leader: Some("node-2".into()),
            voters: Vec::new(),
            source: "node-2".into(),
            correlation: Some("op_72".into()),
            actor: None,
        }
    }

    #[test]
    fn search_reads_the_fields_the_table_shows() {
        let e = event();
        assert!(e.matches(""));
        assert!(e.matches("elected"));
        assert!(e.matches("data-12"));
        assert!(e.matches("node-2"));
        assert!(e.matches("op_72"));
        assert!(!e.matches("metadata"));
    }

    #[test]
    fn a_stamp_drops_the_date_on_the_reference_day() {
        use crate::models::format::epoch;
        let now = epoch(2026, 6, 13, 13, 40, 6);
        assert_eq!(stamp(epoch(2026, 6, 13, 13, 35, 41), now), "13:35:41");
        assert_eq!(stamp(epoch(2026, 6, 12, 9, 41, 3), now), "06-12 09:41:03");
    }

    #[test]
    fn ranges_widen() {
        assert!(ActivityRange::Hour1.seconds() < ActivityRange::Hours24.seconds());
        assert!(ActivityRange::Hours24.seconds() < ActivityRange::Days7.seconds());
    }
}
