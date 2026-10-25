//! Raft group fixtures. Five groups on five members: metadata-0 and
//! sequencer-0 on node-1..3, and three data groups of three voters each.
//! node-5 does not answer, so data-12 and data-13 each hold a quorum of two
//! with one voter missing. Member reachability alone would not show that.

use crate::data::mock::cluster::members::{NOW, node_id, node_name};
use crate::models::cluster::{GroupKind, GroupRole, HostedGroup, ReplicationState};
use crate::models::cluster_raft::{Peer, PeerRole, RaftGroup, SnapshotInfo};

fn peer(n: u32, role: PeerRole, match_index: u64, applied: u64, heartbeat: Option<u64>) -> Peer {
    Peer {
        node_id: node_id(n),
        node: node_name(n),
        role,
        match_index,
        applied_index: applied,
        heartbeat_age_ms: heartbeat,
    }
}

fn snapshot(
    group: &str,
    n: u32,
    index: u64,
    minutes_ago: i64,
    mib: u64,
    node: u32,
) -> SnapshotInfo {
    SnapshotInfo {
        id: format!("snap_{group}_{n}"),
        index,
        taken_at: NOW - minutes_ago * 60,
        size_bytes: mib * 1024 * 1024,
        node: node_name(node),
    }
}

/// The groups as the cluster reports them, in registration order.
pub fn seeded_groups() -> Vec<RaftGroup> {
    use PeerRole::{Leader, Voter};
    vec![
        RaftGroup {
            id: "metadata-0".into(),
            kind: GroupKind::Metadata,
            term: 142,
            commit_index: 1_488_201,
            applied_index: 1_488_201,
            config_index: 1_402_377,
            peers: vec![
                peer(1, Leader, 1_488_201, 1_488_201, Some(0)),
                peer(2, Voter, 1_488_201, 1_488_200, Some(18)),
                peer(3, Voter, 1_488_201, 1_488_201, Some(21)),
            ],
            snapshots: vec![snapshot("metadata-0", 1, 1_480_000, 95, 312, 1)],
        },
        RaftGroup {
            id: "data-11".into(),
            kind: GroupKind::Data,
            term: 87,
            commit_index: 902_417,
            applied_index: 902_415,
            config_index: 311_090,
            peers: vec![
                peer(1, Leader, 902_417, 902_415, Some(0)),
                peer(2, Voter, 902_417, 902_417, Some(22)),
                peer(4, Voter, 901_108, 901_108, Some(41)),
            ],
            snapshots: vec![
                snapshot("data-11", 2, 900_000, 130, 2_480, 1),
                snapshot("data-11", 1, 850_000, 1_460, 2_310, 1),
            ],
        },
        RaftGroup {
            id: "data-12".into(),
            kind: GroupKind::Data,
            term: 91,
            commit_index: 1_204_886,
            applied_index: 1_204_883,
            config_index: 640_512,
            peers: vec![
                peer(2, Leader, 1_204_886, 1_204_883, Some(0)),
                peer(4, Voter, 1_204_006, 1_204_006, Some(412)),
                peer(5, Voter, 1_156_675, 1_156_675, None),
            ],
            snapshots: vec![snapshot("data-12", 1, 1_150_000, 260, 3_120, 2)],
        },
        RaftGroup {
            id: "data-13".into(),
            kind: GroupKind::Data,
            term: 64,
            commit_index: 733_950,
            applied_index: 733_950,
            config_index: 205_338,
            peers: vec![
                peer(3, Leader, 733_950, 733_950, Some(0)),
                peer(4, Voter, 733_950, 733_950, Some(19)),
                peer(5, Voter, 686_020, 686_020, None),
            ],
            snapshots: vec![snapshot("data-13", 1, 720_000, 90, 1_940, 3)],
        },
        RaftGroup {
            id: "sequencer-0".into(),
            kind: GroupKind::Sequencer,
            term: 12,
            commit_index: 48_213,
            applied_index: 48_213,
            config_index: 4_100,
            peers: vec![
                peer(3, Leader, 48_213, 48_213, Some(0)),
                peer(1, Voter, 48_213, 48_213, Some(17)),
                peer(2, Voter, 48_213, 48_212, Some(20)),
            ],
            snapshots: Vec::new(),
        },
    ]
}

/// The groups `node_id` belongs to, as that member observes them. A member
/// that belongs to none hosts none.
pub fn hosted_groups(groups: &[RaftGroup], node_id: &str) -> Vec<HostedGroup> {
    groups
        .iter()
        .filter_map(|g| g.peer(node_id).map(|p| (g, p)))
        .map(|(g, p)| {
            let role_here = match p.role {
                PeerRole::Leader => GroupRole::Leader,
                PeerRole::Voter => GroupRole::Follower,
                PeerRole::Learner => GroupRole::Learner,
            };
            let state = match p.role {
                PeerRole::Leader if g.reachable_voters() < g.voter_count() => {
                    ReplicationState::Voters {
                        available: g.reachable_voters(),
                        total: g.voter_count(),
                    }
                }
                PeerRole::Leader => ReplicationState::CaughtUp,
                PeerRole::Voter | PeerRole::Learner => match g.match_lag(p) {
                    0 => ReplicationState::CaughtUp,
                    entries => ReplicationState::Behind { entries },
                },
            };
            HostedGroup {
                group: g.id.clone(),
                kind: g.kind,
                role_here,
                leader: g.leader_name().to_string(),
                state,
                term: g.term,
                commit_index: g.commit_index,
                applied_index: p.applied_index,
            }
        })
        .collect()
}
