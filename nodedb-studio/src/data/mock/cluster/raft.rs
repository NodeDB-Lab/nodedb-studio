//! Raft group fixtures. Five groups on five members: metadata-0 and
//! sequencer-0 on node-1..3, and three data groups of three voters each.
//! node-5 does not answer, so data-12 and data-13 each hold a quorum of two
//! with one voter missing. Member reachability alone would not show that.

use crate::data::mock::cluster::members::{NOW, link_to, node_id, node_name};
use crate::models::cluster::raft::{Peer, PeerProgress, PeerRole, RaftGroup, SnapshotInfo};
use crate::models::cluster::{GroupKind, GroupRole, HostedGroup, ReplicationState};

fn peer(n: u32, role: PeerRole, match_index: u64, applied: u64, heartbeat: Option<u64>) -> Peer {
    Peer {
        node_id: node_id(n),
        node: node_name(n),
        role,
        progress: Some(PeerProgress {
            match_index,
            applied_index: applied,
            heartbeat_age_ms: heartbeat,
        }),
        link: link_to(n),
    }
}

/// A group whose leader and voter count follow its peers.
fn group(
    id: &str,
    kind: GroupKind,
    (term, commit_index, applied_index, config_index): (u64, u64, u64, u64),
    peers: Vec<Peer>,
    snapshots: Vec<SnapshotInfo>,
) -> RaftGroup {
    RaftGroup {
        id: id.to_string(),
        kind,
        leader: peers
            .iter()
            .find(|p| p.role == PeerRole::Leader)
            .map(|p| p.node.clone()),
        term,
        commit_index,
        applied_index,
        config_index: Some(config_index),
        voter_count: peers.iter().filter(|p| p.role.votes()).count() as u32,
        peers: Some(peers),
        learners_reported: true,
        snapshots: Some(snapshots),
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
        group(
            "metadata-0",
            GroupKind::Metadata,
            (142, 1_488_201, 1_488_201, 1_402_377),
            vec![
                peer(1, Leader, 1_488_201, 1_488_201, Some(0)),
                peer(2, Voter, 1_488_201, 1_488_200, Some(18)),
                peer(3, Voter, 1_488_201, 1_488_201, Some(21)),
            ],
            vec![snapshot("metadata-0", 1, 1_480_000, 95, 312, 1)],
        ),
        group(
            "data-11",
            GroupKind::Data,
            (87, 902_417, 902_415, 311_090),
            vec![
                peer(1, Leader, 902_417, 902_415, Some(0)),
                peer(2, Voter, 902_417, 902_417, Some(22)),
                peer(4, Voter, 901_108, 901_108, Some(41)),
            ],
            vec![
                snapshot("data-11", 2, 900_000, 130, 2_480, 1),
                snapshot("data-11", 1, 850_000, 1_460, 2_310, 1),
            ],
        ),
        group(
            "data-12",
            GroupKind::Data,
            (91, 1_204_886, 1_204_883, 640_512),
            vec![
                peer(2, Leader, 1_204_886, 1_204_883, Some(0)),
                peer(4, Voter, 1_204_006, 1_204_006, Some(412)),
                peer(5, Voter, 1_156_675, 1_156_675, None),
            ],
            vec![snapshot("data-12", 1, 1_150_000, 260, 3_120, 2)],
        ),
        group(
            "data-13",
            GroupKind::Data,
            (64, 733_950, 733_950, 205_338),
            vec![
                peer(3, Leader, 733_950, 733_950, Some(0)),
                peer(4, Voter, 733_950, 733_950, Some(19)),
                peer(5, Voter, 686_020, 686_020, None),
            ],
            vec![snapshot("data-13", 1, 720_000, 90, 1_940, 3)],
        ),
        group(
            "sequencer-0",
            GroupKind::Sequencer,
            (12, 48_213, 48_213, 4_100),
            vec![
                peer(3, Leader, 48_213, 48_213, Some(0)),
                peer(1, Voter, 48_213, 48_213, Some(17)),
                peer(2, Voter, 48_213, 48_212, Some(20)),
            ],
            Vec::new(),
        ),
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
            let up = g.reachable_voters().unwrap_or(0);
            let state = match p.role {
                PeerRole::Leader if up < g.voter_count => ReplicationState::Voters {
                    available: up,
                    total: g.voter_count,
                },
                PeerRole::Leader => ReplicationState::CaughtUp,
                PeerRole::Voter | PeerRole::Learner => match g.match_lag(p).unwrap_or(0) {
                    0 => ReplicationState::CaughtUp,
                    entries => ReplicationState::Behind { entries },
                },
            };
            HostedGroup {
                group: g.id.clone(),
                kind: g.kind,
                role_here,
                leader: g.leader.clone(),
                state: Some(state),
                term: g.term,
                commit_index: g.commit_index,
                applied_index: p.progress.map_or(g.applied_index, |x| x.applied_index),
            }
        })
        .collect()
}
