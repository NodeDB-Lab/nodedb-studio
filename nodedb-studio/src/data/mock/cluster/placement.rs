//! Placement fixtures: nine vShards across three data groups, and the node
//! disks. A vShard's leader, replicas, and unreachable replicas come from its
//! group, so placement and Raft groups never disagree.

use crate::data::mock::cluster::members::{OBSERVED, cluster_members, node_id, observer};
use crate::models::cluster::place::{GIB, GroupQuorum, Movement, Placement, PlacementNode, VShard};
use crate::models::cluster::raft::{PeerRole, RaftGroup};

/// number, database, collection, group, size in GiB.
const SEEDS: &[(u32, &str, &str, &str, u64)] = &[
    (3, "analytics", "orders", "data-11", 18),
    (5, "analytics", "sessions", "data-11", 9),
    (8, "analytics", "orders", "data-13", 32),
    (12, "analytics", "orders", "data-11", 21),
    (14, "analytics", "sessions", "data-12", 11),
    (17, "analytics", "orders", "data-12", 24),
    (21, "analytics", "orders", "data-12", 16),
    (22, "crm", "accounts", "data-13", 6),
    (29, "analytics", "orders", "data-13", 27),
];

/// Disk used per node, in GiB, against 1 TiB.
const DISK_GIB: [u64; 5] = [420, 390, 612, 140, 288];

/// The vShards laid over `groups`. A move in flight stays on its vShard.
pub fn vshards(groups: &[RaftGroup], moving: &[(String, Movement)]) -> Vec<VShard> {
    SEEDS
        .iter()
        .filter_map(|(number, db, coll, group, gib)| {
            let g = groups.iter().find(|g| g.id == *group)?;
            let mut replicas: Vec<String> = g.peers().iter().map(|p| p.node.clone()).collect();
            replicas.sort();
            let unreachable = g
                .peers()
                .iter()
                .filter(|p| p.link.reached() == Some(false))
                .map(|p| p.node.clone())
                .collect();
            Some(VShard {
                number: *number,
                database: Some(db.to_string()),
                collection: Some(coll.to_string()),
                group: group.to_string(),
                leader: g.leader.clone(),
                replicas,
                size_bytes: Some(gib * GIB),
                capacity_bytes: Some(64 * GIB),
                storage_class: Some("standard-ssd".to_string()),
                movement: moving
                    .iter()
                    .find(|(id, _)| id == group)
                    .map(|(_, m)| m.clone()),
                unreachable,
            })
        })
        .collect()
}

/// The placement read: vShards, node disks and roles, and group quorums.
pub fn placement(
    groups: &[RaftGroup],
    moving: &[(String, Movement)],
    revision: Option<u64>,
) -> Placement {
    let vs = vshards(groups, moving);
    let members = cluster_members().members;
    let nodes = members
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let id = node_id(i as u32 + 1);
            let roles: Vec<(String, PeerRole)> = groups
                .iter()
                .filter_map(|g| g.peer(&id).map(|p| (g.id.clone(), p.role)))
                .collect();
            PlacementNode {
                node_id: m.id.clone(),
                name: m.name.clone(),
                state: m.state.clone(),
                link: m.link.clone(),
                vshards: vs.iter().filter(|v| v.replicas.contains(&m.name)).count() as u32,
                disk_used_bytes: DISK_GIB.get(i).map(|g| g * GIB),
                disk_total_bytes: Some(1024 * GIB),
                roles,
            }
        })
        .collect();
    let quorums = groups
        .iter()
        .map(|g| GroupQuorum {
            group: g.id.clone(),
            voters: g.voter_count,
            reachable: g.reachable_voters(),
            quorum: g.quorum(),
        })
        .collect();
    Placement {
        observed_at: OBSERVED,
        revision,
        observer: observer(),
        vshards: vs,
        nodes,
        quorums,
    }
}
