//! Cluster membership and replication, as an Origin cluster reports them.
//!
//! NodeDB runs independent metadata, data, and sequencer Raft groups.
//! Leadership, term, log positions, and quorum belong to a group, never to
//! the cluster: no type here names a cluster-wide leader. Reachability (a
//! node answers) and replication readiness (a group has quorum) are separate
//! facts with separate types.

use serde::{Deserialize, Serialize};

/// Observed reachability of one configured member.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeState {
    Online,
    /// Reachable, but replaying log entries or a snapshot.
    CatchingUp,
    Unreachable,
}

impl NodeState {
    pub fn label(self) -> &'static str {
        match self {
            NodeState::Online => "Online",
            NodeState::CatchingUp => "Catching up",
            NodeState::Unreachable => "Unreachable",
        }
    }

    pub fn reachable(self) -> bool {
        match self {
            NodeState::Online | NodeState::CatchingUp => true,
            NodeState::Unreachable => false,
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

/// One configured cluster member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterMember {
    /// Stable member id, e.g. `nd_02`.
    pub id: String,
    /// Display name, e.g. `node-2`.
    pub name: String,
    pub endpoint: String,
    pub zone: String,
    pub kind: NodeKind,
    pub state: NodeState,
    /// Seconds since the last observation of this member.
    pub last_seen_secs: u64,
    /// Wall-clock time of the last heartbeat, as the server formats it.
    pub last_heartbeat: String,
    pub hosted_groups: u32,
}

/// The member list with the time it was observed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterMembers {
    pub observed_at: String,
    pub members: Vec<ClusterMember>,
}

impl ClusterMembers {
    pub fn reachable(&self) -> usize {
        self.members.iter().filter(|m| m.state.reachable()).count()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroupKind {
    Metadata,
    Data,
    Sequencer,
}

impl GroupKind {
    pub fn label(self) -> &'static str {
        match self {
            GroupKind::Metadata => "Metadata",
            GroupKind::Data => "Data",
            GroupKind::Sequencer => "Sequencer",
        }
    }
}

/// A member's role in one group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroupRole {
    Leader,
    Follower,
    Learner,
}

impl GroupRole {
    pub fn label(self) -> &'static str {
        match self {
            GroupRole::Leader => "Leader",
            GroupRole::Follower => "Follower",
            GroupRole::Learner => "Learner",
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
    /// Group id, e.g. `data-12`.
    pub group: String,
    pub kind: GroupKind,
    pub role_here: GroupRole,
    /// Display name of the group's current leader.
    pub leader: String,
    pub state: ReplicationState,
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

    #[test]
    fn catching_up_counts_as_reachable() {
        assert!(NodeState::CatchingUp.reachable());
        assert!(!NodeState::Unreachable.reachable());
    }
}
