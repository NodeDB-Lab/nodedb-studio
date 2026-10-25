//! Cluster fixtures: five configured Origin members, four reachable, and the
//! Raft groups each one hosts.

use crate::models::cluster::{
    ClusterMember, ClusterMembers, GroupKind, GroupRole, HostedGroup, NodeKind, NodeState,
    ReplicationState,
};

fn member(
    n: u32,
    zone: &str,
    state: NodeState,
    last_seen_secs: u64,
    last_heartbeat: &str,
    hosted_groups: u32,
) -> ClusterMember {
    ClusterMember {
        id: format!("nd_{n:02}"),
        name: format!("node-{n}"),
        endpoint: format!("10.4.0.{}:7443", 10 + n),
        zone: zone.to_string(),
        kind: NodeKind::Origin,
        state,
        last_seen_secs,
        last_heartbeat: last_heartbeat.to_string(),
        hosted_groups,
    }
}

pub fn cluster_members() -> ClusterMembers {
    ClusterMembers {
        observed_at: "13:40:06 UTC".to_string(),
        members: vec![
            member(1, "sg-a", NodeState::Online, 1, "13:40:05 UTC", 3),
            member(2, "sg-b", NodeState::Online, 1, "13:40:05 UTC", 3),
            member(3, "sg-c", NodeState::Online, 2, "13:40:04 UTC", 3),
            member(4, "sg-a", NodeState::CatchingUp, 2, "13:40:04 UTC", 2),
            member(5, "sg-b", NodeState::Unreachable, 240, "13:36:02 UTC", 2),
        ],
    }
}

fn group(
    id: &str,
    kind: GroupKind,
    role_here: GroupRole,
    leader: &str,
    state: ReplicationState,
) -> HostedGroup {
    HostedGroup {
        group: id.to_string(),
        kind,
        role_here,
        leader: leader.to_string(),
        state,
        term: 142,
        commit_index: 1_488_201,
        applied_index: 1_488_201,
    }
}

/// The groups `node_id` hosts, with its role in each. Unknown ids host none.
pub fn hosted_groups(node_id: &str) -> Vec<HostedGroup> {
    use GroupKind as K;
    use GroupRole as R;
    use ReplicationState as S;
    let voters = S::Voters {
        available: 2,
        total: 3,
    };
    match node_id {
        "nd_01" => vec![
            group("metadata-0", K::Metadata, R::Leader, "node-1", S::CaughtUp),
            group("data-11", K::Data, R::Leader, "node-1", S::CaughtUp),
            group(
                "sequencer-0",
                K::Sequencer,
                R::Follower,
                "node-3",
                S::CaughtUp,
            ),
        ],
        "nd_02" => vec![
            group(
                "metadata-0",
                K::Metadata,
                R::Follower,
                "node-1",
                S::CaughtUp,
            ),
            group("data-12", K::Data, R::Leader, "node-2", voters),
            group(
                "sequencer-0",
                K::Sequencer,
                R::Follower,
                "node-3",
                S::CaughtUp,
            ),
        ],
        "nd_03" => vec![
            group(
                "metadata-0",
                K::Metadata,
                R::Follower,
                "node-1",
                S::CaughtUp,
            ),
            group("data-13", K::Data, R::Leader, "node-3", S::CaughtUp),
            group(
                "sequencer-0",
                K::Sequencer,
                R::Leader,
                "node-3",
                S::CaughtUp,
            ),
        ],
        "nd_04" => vec![
            group(
                "data-11",
                K::Data,
                R::Follower,
                "node-1",
                S::Behind { entries: 1_309 },
            ),
            group("data-13", K::Data, R::Follower, "node-3", S::CaughtUp),
        ],
        "nd_05" => vec![
            group(
                "data-12",
                K::Data,
                R::Follower,
                "node-2",
                S::Behind { entries: 48_211 },
            ),
            group(
                "data-13",
                K::Data,
                R::Follower,
                "node-3",
                S::Behind { entries: 47_930 },
            ),
        ],
        _ => Vec::new(),
    }
}
