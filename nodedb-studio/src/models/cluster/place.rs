//! Placement: which Raft group owns each vShard, which nodes hold its
//! replicas, and the plan that moves a replica. A replica move changes the
//! membership of the vShard's group, so it reaches every vShard of that
//! group. Node health (a node answers) and group quorum (voters answer) are
//! separate facts and sit in separate columns. Every value is one node's
//! view: the observer's.

use serde::{Deserialize, Serialize};

use crate::models::cluster::op::{Impact, PermissionCheck};
use crate::models::cluster::raft::PeerRole;
use crate::models::cluster::{Link, NodeState, Observer};
use crate::models::feeds::Tone;

pub const GIB: u64 = 1024 * 1024 * 1024;

/// A replica move that has started and not finished. `None` is a fact the
/// server does not report: NodeDB names only the vShard and its phase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Movement {
    pub operation: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub phase: Option<String>,
}

/// One vShard. `None` is a fact the server does not report: NodeDB hashes
/// rows of every collection across every vShard, so a vShard has no
/// database or collection, and it reports no size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VShard {
    pub number: u32,
    pub database: Option<String>,
    pub collection: Option<String>,
    pub group: String,
    /// The group's leader, by display name. `None`: the observer knows none.
    pub leader: Option<String>,
    /// Display names of the nodes that hold a replica.
    pub replicas: Vec<String>,
    pub size_bytes: Option<u64>,
    pub capacity_bytes: Option<u64>,
    pub storage_class: Option<String>,
    pub movement: Option<Movement>,
    /// Replicas on a node the observer does not reach.
    pub unreachable: Vec<String>,
    /// Replicas on a node the observer cannot say it reaches.
    pub unknown: Vec<String>,
}

/// How a vShard's replicas stand. Placement only: group quorum is read in
/// Raft groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShardStatus {
    /// Every replica sits on a node that answers.
    Placed,
    /// A replica sits on a node that does not answer.
    ReplicaUnreachable,
    /// A replica move is running.
    Moving,
    /// The observer cannot say: it lists no replica for the vShard, or it
    /// cannot tell whether it reaches one.
    Unknown,
}

impl ShardStatus {
    pub fn label(self) -> &'static str {
        match self {
            ShardStatus::Placed => "Placed",
            ShardStatus::ReplicaUnreachable => "Replica unreachable",
            ShardStatus::Moving => "Moving",
            ShardStatus::Unknown => "Not known",
        }
    }

    pub fn tone(self) -> Tone {
        match self {
            ShardStatus::Placed => Tone::Ok,
            ShardStatus::ReplicaUnreachable | ShardStatus::Moving => Tone::Warn,
            ShardStatus::Unknown => Tone::Idle,
        }
    }
}

impl VShard {
    /// `analytics.orders`. `None` when either is not reported.
    pub fn source(&self) -> Option<String> {
        Some(format!(
            "{}.{}",
            self.database.as_ref()?,
            self.collection.as_ref()?
        ))
    }

    pub fn status(&self) -> ShardStatus {
        if self.movement.is_some() {
            ShardStatus::Moving
        } else if !self.unreachable.is_empty() {
            ShardStatus::ReplicaUnreachable
        } else if self.replicas.is_empty() || !self.unknown.is_empty() {
            ShardStatus::Unknown
        } else {
            ShardStatus::Placed
        }
    }

    /// Whole percent of the capacity in use. `None` when either is not
    /// reported or the capacity is zero.
    pub fn used_percent(&self) -> Option<u64> {
        (self.size_bytes? * 100).checked_div(self.capacity_bytes?)
    }
}

/// A node as placement sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlacementNode {
    pub node_id: String,
    pub name: String,
    pub state: NodeState,
    pub link: Link,
    /// Replicas of vShards the node holds.
    pub vshards: u32,
    /// `None`: not reported. NodeDB 0.6.2 reports no disk use.
    pub disk_used_bytes: Option<u64>,
    pub disk_total_bytes: Option<u64>,
    /// The node's role in each group it belongs to.
    pub roles: Vec<(String, PeerRole)>,
}

impl PlacementNode {
    pub fn disk_percent(&self) -> Option<u64> {
        (self.disk_used_bytes? * 100).checked_div(self.disk_total_bytes?)
    }

    pub fn role_in(&self, group: &str) -> Option<PeerRole> {
        self.roles.iter().find(|(g, _)| g == group).map(|(_, r)| *r)
    }
}

/// A group's voter count against its quorum, from the Raft read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupQuorum {
    pub group: String,
    pub voters: u32,
    /// Voters the observer reaches. `None`: it cannot say.
    pub reachable: Option<u32>,
    pub quorum: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placement {
    pub observed_at: i64,
    /// `None`: the server numbers no revision.
    pub revision: Option<u64>,
    pub observer: Observer,
    pub vshards: Vec<VShard>,
    pub nodes: Vec<PlacementNode>,
    pub quorums: Vec<GroupQuorum>,
}

impl Placement {
    pub fn vshard(&self, number: u32) -> Option<&VShard> {
        self.vshards.iter().find(|v| v.number == number)
    }

    pub fn quorum(&self, group: &str) -> Option<&GroupQuorum> {
        self.quorums.iter().find(|q| q.group == group)
    }

    pub fn databases(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .vshards
            .iter()
            .filter_map(|v| v.database.clone())
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// Collections of `database`, or of every database when `None`.
    pub fn collections(&self, database: Option<&str>) -> Vec<String> {
        let mut out: Vec<String> = self
            .vshards
            .iter()
            .filter(|v| database.is_none_or(|d| v.database.as_deref() == Some(d)))
            .filter_map(|v| v.collection.clone())
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// Bytes one replica of `group` holds: every vShard the group owns.
    /// `None` when a vShard's size is not reported.
    pub fn group_bytes(&self, group: &str) -> Option<u64> {
        self.vshards
            .iter()
            .filter(|v| v.group == group)
            .map(|v| v.size_bytes)
            .sum()
    }

    pub fn group_vshards(&self, group: &str) -> Vec<u32> {
        self.vshards
            .iter()
            .filter(|v| v.group == group)
            .map(|v| v.number)
            .collect()
    }
}

/// A replica move the person stages. Names stable ids, not display names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveRequest {
    pub vshard: u32,
    pub from_id: String,
    pub to_id: String,
}

/// One move in a previewed plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedMove {
    pub vshard: u32,
    pub group: String,
    /// `Replica 3`: the replica's place in the group.
    pub replica: String,
    pub source: String,
    pub destination: String,
    pub estimated_bytes: u64,
    pub impact: Impact,
    /// The vShards of the group that move with it.
    pub vshards: Vec<u32>,
}

/// The previewed plan: every move, what it does to each group, and the
/// permission it needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebalancePlan {
    pub moves: Vec<PlannedMove>,
    pub warnings: Vec<String>,
    pub permission: PermissionCheck,
    pub revision: u64,
}

impl RebalancePlan {
    pub fn total_bytes(&self) -> u64 {
        self.moves.iter().map(|m| m.estimated_bytes).sum()
    }

    pub fn can_apply(&self) -> bool {
        self.permission.granted && !self.moves.is_empty()
    }
}

/// `24 GiB`, `1.5 TiB`: a byte count in the unit that reads best.
pub fn size_text(bytes: u64) -> String {
    let gib = bytes as f64 / GIB as f64;
    if gib >= 1024.0 {
        format!("{} TiB", trim(gib / 1024.0))
    } else if gib >= 1.0 {
        format!("{} GiB", trim(gib))
    } else {
        format!("{} MiB", trim(bytes as f64 / (1024.0 * 1024.0)))
    }
}

fn trim(v: f64) -> String {
    let s = format!("{v:.1}");
    s.strip_suffix(".0").map(str::to_string).unwrap_or(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shard(n: u32, group: &str, size: u64) -> VShard {
        VShard {
            number: n,
            database: Some("analytics".into()),
            collection: Some("orders".into()),
            group: group.into(),
            leader: Some("node-1".into()),
            replicas: vec!["node-1".into()],
            size_bytes: Some(size * GIB),
            capacity_bytes: Some(64 * GIB),
            storage_class: Some("standard-ssd".into()),
            movement: None,
            unreachable: Vec::new(),
            unknown: Vec::new(),
        }
    }

    #[test]
    fn sizes_read_in_the_unit_that_fits() {
        assert_eq!(size_text(24 * GIB), "24 GiB");
        assert_eq!(size_text(GIB + GIB / 2), "1.5 GiB");
        assert_eq!(size_text(1024 * GIB), "1 TiB");
        assert_eq!(size_text(512 * 1024 * 1024), "512 MiB");
    }

    #[test]
    fn status_separates_a_move_from_an_unreachable_replica() {
        let mut s = shard(1, "data-1", 10);
        assert_eq!(s.status(), ShardStatus::Placed);
        s.unknown.push("node-4".into());
        assert_eq!(s.status(), ShardStatus::Unknown);
        s.unknown.clear();
        s.replicas.clear();
        assert_eq!(s.status(), ShardStatus::Unknown);
        s.replicas.push("node-1".into());
        s.unreachable.push("node-5".into());
        assert_eq!(s.status(), ShardStatus::ReplicaUnreachable);
        s.movement = Some(Movement {
            operation: Some("op_1".into()),
            from: Some("node-5".into()),
            to: Some("node-1".into()),
            phase: None,
        });
        assert_eq!(s.status(), ShardStatus::Moving);
    }

    #[test]
    fn a_group_move_carries_every_vshard_of_the_group() {
        let p = Placement {
            observed_at: 0,
            revision: Some(1),
            observer: Observer {
                node_id: "nd_01".into(),
            },
            vshards: vec![
                shard(1, "data-1", 10),
                shard(2, "data-1", 5),
                shard(3, "data-2", 7),
            ],
            nodes: Vec::new(),
            quorums: Vec::new(),
        };
        assert_eq!(p.group_bytes("data-1"), Some(15 * GIB));
        assert_eq!(p.group_vshards("data-1"), vec![1, 2]);
        assert_eq!(p.databases(), vec!["analytics".to_string()]);
    }

    #[test]
    fn used_percent_is_whole_and_safe_on_zero_capacity() {
        assert_eq!(shard(1, "g", 24).used_percent(), Some(37));
        let mut s = shard(1, "g", 24);
        s.capacity_bytes = Some(0);
        assert_eq!(s.used_percent(), None);
        s.size_bytes = None;
        s.capacity_bytes = Some(GIB);
        assert_eq!(s.used_percent(), None);
        assert_eq!(s.source().as_deref(), Some("analytics.orders"));
        s.database = None;
        assert_eq!(s.source(), None);
    }
}
