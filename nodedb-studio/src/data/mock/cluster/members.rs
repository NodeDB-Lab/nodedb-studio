//! Cluster fixtures: five configured Origin members, seen from node-1.
//! node-5 stopped answering: its circuit is open and node-1 holds it as
//! `draining`, the way NodeDB reports a dead peer. The
//! Raft groups each one hosts come from the group fixtures, so a member's
//! view and a group's view never disagree.

use crate::models::cluster::{
    Circuit, ClusterMember, ClusterMembers, Link, NodeKind, NodeState, Observer,
};
use crate::models::format::epoch;

/// The fixture clock: the server's answer time.
pub const NOW: i64 = epoch(2026, 6, 13, 13, 40, 0);

/// Seconds past `NOW` at which the members were observed.
pub const OBSERVED: i64 = NOW + 6;

fn member(
    n: u32,
    zone: &str,
    state: NodeState,
    link: Link,
    last_seen_secs: u64,
    last_heartbeat: &str,
    hosted_groups: u32,
) -> ClusterMember {
    ClusterMember {
        id: node_id(n),
        name: node_name(n),
        address: format!("10.4.0.{}:7443", 10 + n),
        zone: Some(zone.to_string()),
        kind: NodeKind::Origin,
        state,
        link,
        last_seen_secs: Some(last_seen_secs),
        last_heartbeat: Some(last_heartbeat.to_string()),
        hosted_groups: Some(hosted_groups),
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

/// The node every fixture read is seen from.
pub fn observer() -> Observer {
    Observer {
        node_id: node_id(1),
    }
}

/// How node-1 reaches node `n`: node-5 stopped answering four minutes ago.
pub fn link_to(n: u32) -> Link {
    match n {
        1 => Link::Observer,
        5 => Link::Circuit {
            state: Circuit::Open,
            failures: 48,
        },
        _ => Link::Circuit {
            state: Circuit::Closed,
            failures: 0,
        },
    }
}

pub fn cluster_members() -> ClusterMembers {
    use NodeState::{Active, Draining};
    ClusterMembers {
        observed_at: "13:40:06 UTC".to_string(),
        observer: observer(),
        members: vec![
            member(1, "sg-a", Active, link_to(1), 1, "13:40:05 UTC", 3),
            member(2, "sg-b", Active, link_to(2), 1, "13:40:05 UTC", 4),
            member(3, "sg-c", Active, link_to(3), 2, "13:40:04 UTC", 3),
            member(4, "sg-a", Active, link_to(4), 2, "13:40:04 UTC", 3),
            member(5, "sg-b", Draining, link_to(5), 240, "13:36:02 UTC", 2),
        ],
    }
}
