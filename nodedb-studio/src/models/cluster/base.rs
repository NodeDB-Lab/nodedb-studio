use serde::{Deserialize, Serialize};

use crate::models::feeds::Tone;

/// The node whose opinion a cluster read is. A connection reaches one node,
/// and during a change each node believes something else: role, leader,
/// term, node state and circuit state are this node's view, never a
/// cluster-wide fact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observer {
    /// The member id of the node that answered.
    pub node_id: String,
}

impl Observer {
    /// `As seen by node 1`.
    pub fn label(&self) -> String {
        format!("As seen by node {}", self.node_id)
    }
}

/// A member's state, in the server's own words (`SHOW NODES`). A peer that
/// stops answering turns `draining` after its ping failures reach a limit:
/// the server never reports a node as dead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeState {
    Active,
    Draining,
    Joining,
    Learner,
    Decommissioned,
    /// A word this version of Studio does not know.
    Unknown(String),
}

impl NodeState {
    /// The server's word. Anything else is `Unknown`, never a guess.
    pub fn parse(text: &str) -> Self {
        match text {
            "active" => NodeState::Active,
            "draining" => NodeState::Draining,
            "joining" => NodeState::Joining,
            "learner" => NodeState::Learner,
            "decommissioned" => NodeState::Decommissioned,
            other => NodeState::Unknown(other.to_string()),
        }
    }

    /// The server's word, unchanged.
    pub fn label(&self) -> &str {
        match self {
            NodeState::Active => "active",
            NodeState::Draining => "draining",
            NodeState::Joining => "joining",
            NodeState::Learner => "learner",
            NodeState::Decommissioned => "decommissioned",
            NodeState::Unknown(s) => s,
        }
    }

    pub fn tone(&self) -> Tone {
        match self {
            NodeState::Active => Tone::Ok,
            NodeState::Draining | NodeState::Joining | NodeState::Learner => Tone::Warn,
            NodeState::Decommissioned | NodeState::Unknown(_) => Tone::Idle,
        }
    }
}

/// The observer's circuit breaker to a peer, in the server's own words
/// (`SHOW PEER HEALTH`). It opens after repeated failed calls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Circuit {
    Closed,
    Open,
    HalfOpen,
    /// A word this version of Studio does not know.
    Unknown(String),
}

impl Circuit {
    /// The server's word. Anything else is `Unknown`, never a guess.
    pub fn parse(text: &str) -> Self {
        match text {
            "closed" => Circuit::Closed,
            "OPEN" => Circuit::Open,
            "half-open" => Circuit::HalfOpen,
            other => Circuit::Unknown(other.to_string()),
        }
    }

    /// The server's word, unchanged.
    pub fn label(&self) -> &str {
        match self {
            Circuit::Closed => "closed",
            Circuit::Open => "OPEN",
            Circuit::HalfOpen => "half-open",
            Circuit::Unknown(s) => s,
        }
    }
}

/// How the observer reaches a member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Link {
    /// The member is the observer itself.
    Observer,
    /// The observer's circuit to the member, and its failed calls in a row.
    Circuit { state: Circuit, failures: u64 },
    /// The observer does not report this member.
    NotReported,
}

impl Link {
    /// Whether the observer reaches the member: `None` when it cannot say.
    /// A half-open circuit is still probing after failures, so it counts
    /// as not reached.
    pub fn reached(&self) -> Option<bool> {
        match self {
            Link::Observer => Some(true),
            Link::Circuit { state, .. } => match state {
                Circuit::Closed => Some(true),
                Circuit::Open | Circuit::HalfOpen => Some(false),
                Circuit::Unknown(_) => None,
            },
            Link::NotReported => None,
        }
    }

    /// `closed`, `OPEN · 5 failures`, `This node`, `Not reported`.
    pub fn label(&self) -> String {
        match self {
            Link::Observer => "This node".to_string(),
            Link::Circuit { state, failures: 0 } => state.label().to_string(),
            Link::Circuit { state, failures } => {
                format!("{} · {failures} failed", state.label())
            }
            Link::NotReported => "Not reported".to_string(),
        }
    }

    pub fn tone(&self) -> Tone {
        match self.reached() {
            Some(true) => Tone::Ok,
            Some(false) => Tone::Err,
            None => Tone::Idle,
        }
    }
}

/// What a member runs. Lite devices synchronize through Sync and never
/// appear as cluster members.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    Origin,
}

impl NodeKind {
    pub fn label(self) -> &'static str {
        match self {
            NodeKind::Origin => "Origin",
        }
    }
}

/// One configured cluster member, as the observer sees it. `None` is a
/// fact the server does not report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterMember {
    /// Stable member id, e.g. `nd_02`, or `2` on NodeDB.
    pub id: String,
    /// Display name. NodeDB reports none, so it is `node <id>` there.
    pub name: String,
    /// The address the member's peers reach it at. On NodeDB this is the
    /// Raft transport address, not the one a client connects to.
    pub address: String,
    pub zone: Option<String>,
    pub kind: NodeKind,
    pub state: NodeState,
    pub link: Link,
    /// Seconds since the last observation of this member.
    pub last_seen_secs: Option<u64>,
    /// Wall-clock time of the last heartbeat, as the server formats it.
    pub last_heartbeat: Option<String>,
    pub hosted_groups: Option<u32>,
}

/// The member list, whose view it is, and when Studio read it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterMembers {
    pub observed_at: String,
    pub observer: Observer,
    pub members: Vec<ClusterMember>,
}

impl ClusterMembers {
    /// Members the observer holds as `active`.
    pub fn active(&self) -> usize {
        self.members
            .iter()
            .filter(|m| m.state == NodeState::Active)
            .count()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroupKind {
    Metadata,
    Data,
    Sequencer,
}

/// NodeDB's Calvin sequencer group, `0xFFFF_FFFF_0000_0001`
/// (`nodedb-cluster` `SEQUENCER_GROUP_ID`). SQL prints it cast to `i64`,
/// as `-4294967295`.
pub const SEQUENCER_GROUP_ID: u64 = 0xFFFF_FFFF_0000_0001;

impl GroupKind {
    pub fn label(self) -> &'static str {
        match self {
            GroupKind::Metadata => "Metadata",
            GroupKind::Data => "Data",
            GroupKind::Sequencer => "Sequencer",
        }
    }

    /// The kind of a NodeDB group id. The server sends no kind: group 0 is
    /// the metadata group, the sequencer has its fixed id, and every other
    /// group is a data group.
    pub fn of_id(id: u64) -> Self {
        match id {
            0 => GroupKind::Metadata,
            SEQUENCER_GROUP_ID => GroupKind::Sequencer,
            _ => GroupKind::Data,
        }
    }
}

/// A member's role in one group, in the server's own words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroupRole {
    Leader,
    Follower,
    Candidate,
    Learner,
    /// Receives a source cluster's log and never votes.
    Observer,
    /// A word this version of Studio does not know.
    Unknown(String),
}

impl GroupRole {
    /// The server's word. Anything else is `Unknown`, never a guess.
    pub fn parse(text: &str) -> Self {
        match text {
            "Leader" => GroupRole::Leader,
            "Follower" => GroupRole::Follower,
            "Candidate" => GroupRole::Candidate,
            "Learner" => GroupRole::Learner,
            "Observer" => GroupRole::Observer,
            other => GroupRole::Unknown(other.to_string()),
        }
    }

    pub fn label(&self) -> &str {
        match self {
            GroupRole::Leader => "Leader",
            GroupRole::Follower => "Follower",
            GroupRole::Candidate => "Candidate",
            GroupRole::Learner => "Learner",
            GroupRole::Observer => "Observer",
            GroupRole::Unknown(s) => s,
        }
    }
}

/// Replication readiness of one group, as observed from one member.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplicationState {
    /// This member's log matches the leader's commit index.
    CaughtUp,
    /// Seen from the leader: how many voters answer.
    Voters { available: u32, total: u32 },
    /// This member trails the leader's commit index.
    Behind { entries: u64 },
}

impl ReplicationState {
    pub fn label(self) -> String {
        match self {
            ReplicationState::CaughtUp => "Caught up".to_string(),
            ReplicationState::Voters { available, total } => {
                format!("{available} of {total} voters available")
            }
            ReplicationState::Behind { entries } => format!("{entries} entries behind"),
        }
    }

    /// Whether the state needs attention: a lost quorum or a lagging log.
    pub fn healthy(self) -> bool {
        match self {
            ReplicationState::CaughtUp => true,
            ReplicationState::Voters { available, total } => available * 2 > total,
            ReplicationState::Behind { .. } => false,
        }
    }
}

/// One Raft group hosted on a member, with that member's view of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostedGroup {
    /// Group id, e.g. `data-12`, or `2` on NodeDB.
    pub group: String,
    pub kind: GroupKind,
    pub role_here: GroupRole,
    /// Display name of the leader this member knows. `None`: it knows none,
    /// as during an election.
    pub leader: Option<String>,
    /// `None`: not reported. NodeDB reports no member's lag behind its
    /// leader.
    pub state: Option<ReplicationState>,
    pub term: u64,
    pub commit_index: u64,
    pub applied_index: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voters_need_a_majority_to_be_healthy() {
        assert!(
            ReplicationState::Voters {
                available: 2,
                total: 3
            }
            .healthy()
        );
        assert!(
            !ReplicationState::Voters {
                available: 1,
                total: 3
            }
            .healthy()
        );
    }

    /// Every state `node_state_str` in NodeDB 0.6.2 can print
    /// (`control/server/shared/ddl/neutral/cluster/support.rs`).
    #[test]
    fn every_node_state_the_server_prints_is_known() {
        for word in ["joining", "active", "draining", "learner", "decommissioned"] {
            let s = NodeState::parse(word);
            assert!(!matches!(s, NodeState::Unknown(_)), "{word}");
            assert_eq!(s.label(), word);
        }
        assert_eq!(
            NodeState::parse("Active"),
            NodeState::Unknown("Active".into())
        );
        assert_eq!(NodeState::parse("dead").label(), "dead");
    }

    /// Every circuit word `SHOW PEER HEALTH` prints in NodeDB 0.6.2
    /// (`control/server/shared/ddl/neutral/cluster/health.rs`).
    #[test]
    fn every_circuit_word_the_server_prints_is_known() {
        for word in ["closed", "OPEN", "half-open"] {
            let c = Circuit::parse(word);
            assert!(!matches!(c, Circuit::Unknown(_)), "{word}");
            assert_eq!(c.label(), word);
        }
        assert_eq!(Circuit::parse("open"), Circuit::Unknown("open".into()));
    }

    /// Every role `GroupStatus.role` prints: `format!("{:?}", NodeRole)` over
    /// `nodedb-raft` `NodeRole`.
    #[test]
    fn every_group_role_the_server_prints_is_known() {
        for word in ["Leader", "Follower", "Candidate", "Learner", "Observer"] {
            let r = GroupRole::parse(word);
            assert!(!matches!(r, GroupRole::Unknown(_)), "{word}");
            assert_eq!(r.label(), word);
        }
        assert_eq!(
            GroupRole::parse("leader"),
            GroupRole::Unknown("leader".into())
        );
    }

    #[test]
    fn a_link_reaches_only_on_a_closed_circuit_or_itself() {
        let c = |state| Link::Circuit { state, failures: 0 };
        assert_eq!(Link::Observer.reached(), Some(true));
        assert_eq!(c(Circuit::Closed).reached(), Some(true));
        assert_eq!(c(Circuit::Open).reached(), Some(false));
        assert_eq!(c(Circuit::HalfOpen).reached(), Some(false));
        assert_eq!(c(Circuit::Unknown("x".into())).reached(), None);
        assert_eq!(Link::NotReported.reached(), None);
    }

    #[test]
    fn group_kind_follows_the_server_ids() {
        assert_eq!(GroupKind::of_id(0), GroupKind::Metadata);
        assert_eq!(GroupKind::of_id(3), GroupKind::Data);
        // SQL prints the sequencer id as an i64.
        assert_eq!(
            GroupKind::of_id((-4_294_967_295_i64) as u64),
            GroupKind::Sequencer
        );
    }

    #[test]
    fn the_observer_names_itself() {
        let o = Observer {
            node_id: "2".into(),
        };
        assert_eq!(o.label(), "As seen by node 2");
    }
}
