//! Cluster fixtures: five configured Origin members, four reachable. The
//! Raft groups each one hosts come from the group fixtures, so a member's
//! view and a group's view never disagree.

use crate::models::cluster::{ClusterMember, ClusterMembers, NodeKind, NodeState};
use crate::models::format::epoch;

/// The fixture clock: the server's answer time.
pub const NOW: i64 = epoch(2026, 6, 13, 13, 40, 0);

/// Seconds past `NOW` at which the members were observed.
pub const OBSERVED: i64 = NOW + 6;

fn member(
    n: u32,
    zone: &str,
    state: NodeState,
    last_seen_secs: u64,
    last_heartbeat: &str,
    hosted_groups: u32,
) -> ClusterMember {
    ClusterMember {
        id: node_id(n),
        name: node_name(n),
        endpoint: format!("10.4.0.{}:7443", 10 + n),
        zone: zone.to_string(),
        kind: NodeKind::Origin,
        state,
        last_seen_secs,
        last_heartbeat: last_heartbeat.to_string(),
        hosted_groups,
    }
}

/// `nd_02`.
pub fn node_id(n: u32) -> String {
    format!("nd_{n:02}")
}

/// `node-2`.
pub fn node_name(n: u32) -> String {
    format!("node-{n}")
}

pub fn cluster_members() -> ClusterMembers {
    ClusterMembers {
        observed_at: "13:40:06 UTC".to_string(),
        members: vec![
            member(1, "sg-a", NodeState::Online, 1, "13:40:05 UTC", 3),
            member(2, "sg-b", NodeState::Online, 1, "13:40:05 UTC", 4),
            member(3, "sg-c", NodeState::Online, 2, "13:40:04 UTC", 3),
            member(4, "sg-a", NodeState::CatchingUp, 2, "13:40:04 UTC", 3),
            member(5, "sg-b", NodeState::Unreachable, 240, "13:36:02 UTC", 2),
        ],
    }
}
