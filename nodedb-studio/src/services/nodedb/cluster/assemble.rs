//! Studio's cluster models built from decoded rows of one observer. Pure,
//! so every rule is tested without a server. A fact the rows do not hold
//! is `None`, never a default.

use std::collections::{BTreeMap, HashMap};

use crate::models::cluster::op::quorum_of;
use crate::models::cluster::place::{GroupQuorum, Movement, Placement, PlacementNode, VShard};
use crate::models::cluster::raft::{Peer, PeerRole, RaftGroup};
use crate::models::cluster::{
    ClusterMember, ClusterMembers, GroupKind, HostedGroup, Link, NodeKind, Observer,
};
use crate::services::nodedb::cluster::decode::{GroupRow, NodeRow, RangeRow, Seen};

/// `node 2`: NodeDB names no member, so Studio names it by its id.
pub(in crate::services::nodedb) fn node_name(id: u64) -> String {
    format!("node {id}")
}

pub(in crate::services::nodedb) fn observer(seen: &Seen) -> Observer {
    Observer {
        node_id: seen.node_id.to_string(),
    }
}

/// The leader a group row names. `0` is none.
fn leader(id: u64) -> Option<String> {
    (id != 0).then(|| node_name(id))
}

/// How the observer reaches node `id`.
fn link(seen: &Seen, links: &HashMap<u64, Link>, id: u64) -> Link {
    if id == seen.node_id {
        Link::Observer
    } else {
        links.get(&id).cloned().unwrap_or(Link::NotReported)
    }
}

/// The members, in node id order. Only the observer's own hosted groups
/// are known: `SHOW NODES` leaves `raft_groups` blank.
pub(in crate::services::nodedb) fn members(
    seen: &Seen,
    nodes: &[NodeRow],
    links: &HashMap<u64, Link>,
    observed_at: String,
) -> ClusterMembers {
    let mut sorted: Vec<&NodeRow> = nodes.iter().collect();
    sorted.sort_by_key(|n| n.node_id);
    ClusterMembers {
        observed_at,
        observer: observer(seen),
        members: sorted
            .into_iter()
            .map(|n| ClusterMember {
                id: n.node_id.to_string(),
                name: node_name(n.node_id),
                address: n.address.clone(),
                zone: None,
                kind: NodeKind::Origin,
                state: n.state.clone(),
                link: link(seen, links, n.node_id),
                last_seen_secs: None,
                last_heartbeat: None,
                hosted_groups: (n.node_id == seen.node_id).then_some(seen.hosted),
            })
            .collect(),
    }
}

/// The observer's groups as it holds them. It reports no lag behind a
/// leader, so the replication state is not reported.
pub(in crate::services::nodedb) fn hosted(rows: &[GroupRow]) -> Vec<HostedGroup> {
    rows.iter()
        .map(|g| HostedGroup {
            group: g.group_id.to_string(),
            kind: GroupKind::of_id(g.group_id),
            role_here: g.role.clone(),
            leader: leader(g.leader_id),
            state: None,
            term: g.term,
            commit_index: g.commit_index,
            applied_index: g.last_applied,
        })
        .collect()
}

/// One group. `members` are the voters of the observer's routing table:
/// the one the observer believes leads is the leader, the rest are voters.
/// Learners, log progress, the configuration index and snapshots are not
/// reported.
pub(in crate::services::nodedb) fn raft_group(
    seen: &Seen,
    row: &GroupRow,
    members: Option<Vec<u64>>,
    links: &HashMap<u64, Link>,
) -> RaftGroup {
    let peers = members.map(|ids| {
        ids.into_iter()
            .map(|id| Peer {
                node_id: id.to_string(),
                node: node_name(id),
                role: if id == row.leader_id {
                    PeerRole::Leader
                } else {
                    PeerRole::Voter
                },
                progress: None,
                link: link(seen, links, id),
            })
            .collect()
    });
    RaftGroup {
        id: row.group_id.to_string(),
        kind: GroupKind::of_id(row.group_id),
        leader: leader(row.leader_id),
        term: row.term,
        commit_index: row.commit_index,
        applied_index: row.last_applied,
        config_index: None,
        voter_count: row.members,
        peers,
        learners_reported: false,
        snapshots: None,
    }
}

/// Every vShard from the observer's routing table, the nodes, and the
/// quorum of each data group from the observer's links. NodeDB ties no
/// vShard to a collection and reports no size or disk.
pub(in crate::services::nodedb) fn placement(
    seen: &Seen,
    nodes: &[NodeRow],
    links: &HashMap<u64, Link>,
    ranges: &[RangeRow],
    moving: &HashMap<u32, String>,
    observed_at: i64,
) -> Placement {
    let down = |id: u64| link(seen, links, id).reached() == Some(false);
    let vshards = ranges
        .iter()
        .map(|r| VShard {
            number: r.vshard,
            database: None,
            collection: None,
            group: r.group_id.to_string(),
            leader: leader(r.leaseholder),
            replicas: r.replicas.iter().map(|id| node_name(*id)).collect(),
            size_bytes: None,
            capacity_bytes: None,
            storage_class: None,
            movement: moving.get(&r.vshard).map(|phase| Movement {
                operation: None,
                from: None,
                to: None,
                phase: Some(phase.clone()),
            }),
            unreachable: r
                .replicas
                .iter()
                .filter(|id| down(**id))
                .map(|id| node_name(*id))
                .collect(),
        })
        .collect();
    // One row per data group: every vShard of a group shares its replicas.
    let groups: BTreeMap<u64, &RangeRow> = ranges.iter().map(|r| (r.group_id, r)).collect();
    let mut sorted: Vec<&NodeRow> = nodes.iter().collect();
    sorted.sort_by_key(|n| n.node_id);
    let nodes = sorted
        .into_iter()
        .map(|n| PlacementNode {
            node_id: n.node_id.to_string(),
            name: node_name(n.node_id),
            state: n.state.clone(),
            link: link(seen, links, n.node_id),
            vshards: ranges
                .iter()
                .filter(|r| r.replicas.contains(&n.node_id))
                .count() as u32,
            disk_used_bytes: None,
            disk_total_bytes: None,
            roles: groups
                .values()
                .filter(|r| r.replicas.contains(&n.node_id))
                .map(|r| {
                    let role = if r.leaseholder == n.node_id {
                        PeerRole::Leader
                    } else {
                        PeerRole::Voter
                    };
                    (r.group_id.to_string(), role)
                })
                .collect(),
        })
        .collect();
    let quorums = groups
        .values()
        .map(|r| {
            let voters = r.replicas.len() as u32;
            let reachable = r
                .replicas
                .iter()
                .map(|id| link(seen, links, *id).reached())
                .try_fold(0, |n, up| up.map(|up| n + u32::from(up)));
            GroupQuorum {
                group: r.group_id.to_string(),
                voters,
                reachable,
                quorum: quorum_of(voters),
            }
        })
        .collect();
    Placement {
        observed_at,
        revision: None,
        observer: observer(seen),
        vshards,
        nodes,
        quorums,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::cluster::raft::GroupHealth;
    use crate::models::cluster::{Circuit, GroupRole, NodeState, SEQUENCER_GROUP_ID};

    fn seen() -> Seen {
        Seen {
            node_id: 1,
            hosted: 6,
        }
    }

    fn node(id: u64, state: NodeState) -> NodeRow {
        NodeRow {
            node_id: id,
            address: format!("127.0.0.1:{id}6400"),
            state,
        }
    }

    /// Node 3 stopped: node 1's circuit to it is open, and node 1 still
    /// holds it as active.
    fn links() -> HashMap<u64, Link> {
        HashMap::from([
            (
                2,
                Link::Circuit {
                    state: Circuit::Closed,
                    failures: 0,
                },
            ),
            (
                3,
                Link::Circuit {
                    state: Circuit::Open,
                    failures: 5,
                },
            ),
        ])
    }

    fn row(group_id: u64, leader_id: u64) -> GroupRow {
        GroupRow {
            group_id,
            role: GroupRole::Follower,
            leader_id,
            term: 2,
            commit_index: 12,
            last_applied: 11,
            members: 3,
        }
    }

    #[test]
    fn members_keep_state_and_circuit_apart_and_name_the_observer() {
        let nodes = [
            node(3, NodeState::Active),
            node(1, NodeState::Active),
            node(2, NodeState::Active),
        ];
        let m = members(&seen(), &nodes, &links(), "10:00:00 UTC".into());
        assert_eq!(m.observer.label(), "As seen by node 1");
        let ids: Vec<_> = m.members.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["1", "2", "3"]);
        assert_eq!(m.members[0].link, Link::Observer);
        assert_eq!(m.members[0].hosted_groups, Some(6));
        // Node state and circuit stay separate: active, yet not reached.
        assert_eq!(m.members[2].state, NodeState::Active);
        assert_eq!(m.members[2].link.reached(), Some(false));
        assert_eq!(m.members[2].hosted_groups, None);
        assert_eq!(m.members[2].zone, None);
        assert_eq!(m.members[2].last_seen_secs, None);
    }

    #[test]
    fn a_member_missing_from_peer_health_is_not_reported() {
        let m = members(
            &seen(),
            &[node(4, NodeState::Joining)],
            &links(),
            String::new(),
        );
        assert_eq!(m.members[0].link, Link::NotReported);
    }

    #[test]
    fn a_single_node_server_is_one_member_and_no_peer() {
        let m = members(
            &seen(),
            &[node(1, NodeState::Active)],
            &HashMap::new(),
            String::new(),
        );
        assert_eq!(m.members.len(), 1);
        assert_eq!(m.members[0].link, Link::Observer);
        let one = GroupRow {
            members: 1,
            ..row(2, 1)
        };
        let g = raft_group(&seen(), &one, Some(vec![1]), &HashMap::new());
        assert_eq!(g.health(), Some(GroupHealth::Healthy));
        assert_eq!(g.quorum(), 1);
    }

    #[test]
    fn a_group_is_the_observers_view_with_progress_not_reported() {
        let g = raft_group(&seen(), &row(2, 2), Some(vec![1, 3, 2]), &links());
        assert_eq!(g.leader.as_deref(), Some("node 2"));
        assert_eq!(g.leader().map(|p| p.node_id.as_str()), Some("2"));
        assert_eq!(g.peers().len(), 3);
        assert!(g.peers().iter().all(|p| p.progress.is_none()));
        assert_eq!(g.config_index, None);
        assert_eq!(g.snapshots, None);
        assert!(!g.learners_reported);
        // Node 1 reaches itself and node 2, not node 3.
        assert_eq!(g.reachable_voters(), Some(2));
        assert_eq!(g.health(), Some(GroupHealth::Degraded));
    }

    #[test]
    fn the_sequencer_without_members_has_no_health() {
        let g = raft_group(&seen(), &row(SEQUENCER_GROUP_ID, 1), None, &links());
        assert_eq!(g.kind, GroupKind::Sequencer);
        assert_eq!(g.id, "18446744069414584321");
        assert_eq!(g.peers, None);
        assert_eq!(g.voter_count, 3);
        assert_eq!(g.health(), None);
        assert_eq!(g.leader.as_deref(), Some("node 1"));
    }

    #[test]
    fn no_leader_is_none_never_a_node_zero() {
        let g = raft_group(&seen(), &row(2, 0), Some(vec![1, 2]), &links());
        assert_eq!(g.leader, None);
        assert_eq!(g.leader_name(), "none");
        assert!(g.leader().is_none());
        assert_eq!(hosted(&[row(2, 0)])[0].leader, None);
    }

    #[test]
    fn hosted_groups_report_no_replication_state() {
        let h = hosted(&[row(0, 1), row(4, 3)]);
        assert_eq!(h[0].kind, GroupKind::Metadata);
        assert_eq!(h[1].leader.as_deref(), Some("node 3"));
        assert!(h.iter().all(|g| g.state.is_none()));
        assert_eq!(h[1].applied_index, 11);
    }

    #[test]
    fn placement_follows_the_routing_and_reports_no_sizes() {
        let ranges = [
            RangeRow {
                vshard: 0,
                group_id: 1,
                leaseholder: 3,
                replicas: vec![1, 3, 2],
            },
            RangeRow {
                vshard: 1,
                group_id: 2,
                leaseholder: 0,
                replicas: vec![1, 2],
            },
        ];
        let nodes = [node(1, NodeState::Active), node(3, NodeState::Draining)];
        let moving = HashMap::from([(1, "copy".to_string())]);
        let p = placement(&seen(), &nodes, &links(), &ranges, &moving, 5);
        assert_eq!(p.observer.node_id, "1");
        assert_eq!(p.revision, None);
        let v0 = p.vshard(0).expect("vshard 0");
        assert_eq!(v0.leader.as_deref(), Some("node 3"));
        assert_eq!(v0.unreachable, ["node 3"]);
        assert_eq!(v0.source(), None);
        assert_eq!(v0.size_bytes, None);
        let v1 = p.vshard(1).expect("vshard 1");
        assert_eq!(v1.leader, None);
        assert_eq!(
            v1.movement.as_ref().and_then(|m| m.phase.as_deref()),
            Some("copy")
        );
        assert_eq!(p.nodes[1].roles, [("1".to_string(), PeerRole::Leader)]);
        assert_eq!(p.nodes[0].vshards, 2);
        assert_eq!(p.nodes[0].disk_used_bytes, None);
        let q1 = p.quorum("1").expect("group 1");
        assert_eq!((q1.voters, q1.reachable, q1.quorum), (3, Some(2), 2));
        assert!(p.databases().is_empty());
    }
}
