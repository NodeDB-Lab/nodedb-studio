//! Raft groups as an Origin cluster reports them: one group, its peers, and
//! its log positions. Leadership, term, quorum, and every index belong to a
//! group. Index lag counts entries and heartbeat age counts milliseconds:
//! they are separate measures with separate fields and separate columns.

use serde::{Deserialize, Serialize};

use crate::models::cluster::GroupKind;
use crate::models::cluster_op::{PermissionCheck, quorum_of};
use crate::models::feeds::Tone;

/// A peer's role in one group. A learner replicates but never votes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerRole {
    Leader,
    Voter,
    Learner,
}

impl PeerRole {
    pub fn label(self) -> &'static str {
        match self {
            PeerRole::Leader => "Leader",
            PeerRole::Voter => "Voter",
            PeerRole::Learner => "Learner",
        }
    }

    pub fn votes(self) -> bool {
        match self {
            PeerRole::Leader | PeerRole::Voter => true,
            PeerRole::Learner => false,
        }
    }
}

/// Heartbeat above this age reads as delayed.
pub const DELAYED_MS: u64 = 300;

/// What the leader observes of one peer's heartbeat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerState {
    Alive,
    Delayed,
    Unreachable,
}

impl PeerState {
    pub fn label(self) -> &'static str {
        match self {
            PeerState::Alive => "Alive",
            PeerState::Delayed => "Delayed",
            PeerState::Unreachable => "Unreachable",
        }
    }

    pub fn tone(self) -> Tone {
        match self {
            PeerState::Alive => Tone::Ok,
            PeerState::Delayed => Tone::Warn,
            PeerState::Unreachable => Tone::Idle,
        }
    }

    pub fn reachable(self) -> bool {
        match self {
            PeerState::Alive | PeerState::Delayed => true,
            PeerState::Unreachable => false,
        }
    }

    /// The state a heartbeat age implies. No age is an unreachable peer.
    pub fn from_age(age_ms: Option<u64>) -> Self {
        match age_ms {
            None => PeerState::Unreachable,
            Some(a) if a >= DELAYED_MS => PeerState::Delayed,
            Some(_) => PeerState::Alive,
        }
    }
}

/// How a group is doing, from its voters alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroupHealth {
    /// Every voter answers.
    Healthy,
    /// A voter is unreachable, but a quorum still answers.
    Degraded,
    /// Fewer voters answer than a quorum needs: the group cannot commit.
    NoQuorum,
}

impl GroupHealth {
    pub fn label(self) -> &'static str {
        match self {
            GroupHealth::Healthy => "Healthy",
            GroupHealth::Degraded => "Degraded",
            GroupHealth::NoQuorum => "No quorum",
        }
    }

    pub fn tone(self) -> Tone {
        match self {
            GroupHealth::Healthy => Tone::Ok,
            GroupHealth::Degraded => Tone::Warn,
            GroupHealth::NoQuorum => Tone::Err,
        }
    }
}

/// One member of a group, as the group's leader observes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Peer {
    pub node_id: String,
    /// Display name, e.g. `node-4`.
    pub node: String,
    pub role: PeerRole,
    /// The highest log index the leader knows this peer holds.
    pub match_index: u64,
    /// The highest index this peer has applied to its state machine.
    pub applied_index: u64,
    /// Milliseconds since the last heartbeat answer. `None` is no answer.
    pub heartbeat_age_ms: Option<u64>,
}

impl Peer {
    pub fn state(&self) -> PeerState {
        PeerState::from_age(self.heartbeat_age_ms)
    }
}

/// A snapshot of one group's state machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotInfo {
    pub id: String,
    /// The last log index the snapshot covers.
    pub index: u64,
    pub taken_at: i64,
    pub size_bytes: u64,
    /// The node that took it.
    pub node: String,
}

/// One Raft group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaftGroup {
    /// Group id, e.g. `data-12`.
    pub id: String,
    pub kind: GroupKind,
    pub term: u64,
    pub commit_index: u64,
    /// The leader's applied index.
    pub applied_index: u64,
    /// The log index of the group's current membership configuration.
    pub config_index: u64,
    pub peers: Vec<Peer>,
    pub snapshots: Vec<SnapshotInfo>,
}

impl RaftGroup {
    pub fn leader(&self) -> Option<&Peer> {
        self.peers.iter().find(|p| p.role == PeerRole::Leader)
    }

    pub fn leader_name(&self) -> &str {
        self.leader().map(|p| p.node.as_str()).unwrap_or("none")
    }

    pub fn voters(&self) -> Vec<&Peer> {
        self.peers.iter().filter(|p| p.role.votes()).collect()
    }

    pub fn learners(&self) -> Vec<&Peer> {
        self.peers.iter().filter(|p| !p.role.votes()).collect()
    }

    pub fn voter_count(&self) -> u32 {
        self.voters().len() as u32
    }

    pub fn quorum(&self) -> u32 {
        quorum_of(self.voter_count())
    }

    /// Voters whose heartbeat answers. The leader counts as itself.
    pub fn reachable_voters(&self) -> u32 {
        self.voters()
            .iter()
            .filter(|p| p.role == PeerRole::Leader || p.state().reachable())
            .count() as u32
    }

    pub fn health(&self) -> GroupHealth {
        let (up, total) = (self.reachable_voters(), self.voter_count());
        if up < self.quorum() {
            GroupHealth::NoQuorum
        } else if up < total {
            GroupHealth::Degraded
        } else {
            GroupHealth::Healthy
        }
    }

    /// Committed entries the leader has not applied yet.
    pub fn pending_entries(&self) -> u64 {
        self.commit_index.saturating_sub(self.applied_index)
    }

    /// Entries a peer's match index trails the commit index by.
    pub fn match_lag(&self, peer: &Peer) -> u64 {
        self.commit_index.saturating_sub(peer.match_index)
    }

    pub fn peer(&self, node_id: &str) -> Option<&Peer> {
        self.peers.iter().find(|p| p.node_id == node_id)
    }

    /// Voters other than the leader that answer: the members leadership can
    /// move to.
    pub fn transfer_targets(&self) -> Vec<&Peer> {
        self.peers
            .iter()
            .filter(|p| p.role == PeerRole::Voter && p.state().reachable())
            .collect()
    }
}

/// Every group with the time it was observed and the revision a write must
/// name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaftGroups {
    pub observed_at: i64,
    pub revision: u64,
    pub groups: Vec<RaftGroup>,
}

impl RaftGroups {
    pub fn group(&self, id: &str) -> Option<&RaftGroup> {
        self.groups.iter().find(|g| g.id == id)
    }

    /// The group to show when none is chosen, or the chosen one is gone: the
    /// first group that needs attention, else the first group.
    pub fn default_group(&self) -> Option<&RaftGroup> {
        self.groups
            .iter()
            .find(|g| g.health() != GroupHealth::Healthy)
            .or_else(|| self.groups.first())
    }
}

/// What a leadership transfer will do, read before anything changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferPreview {
    pub group: String,
    pub kind: GroupKind,
    pub term: u64,
    pub from_id: String,
    pub from: String,
    pub to_id: String,
    pub to: String,
    /// Entries the target trails the commit index by.
    pub target_lag: u64,
    pub voters_reachable: u32,
    pub voters_total: u32,
    /// What changes, one fact each.
    pub effects: Vec<String>,
    /// What the person must weigh before applying.
    pub warnings: Vec<String>,
    pub permission: PermissionCheck,
    /// The revision the preview read. Apply names it.
    pub revision: u64,
}

impl TransferPreview {
    /// Whether the review may offer Apply.
    pub fn can_apply(&self) -> bool {
        self.permission.granted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(node: &str, role: PeerRole, m: u64, hb: Option<u64>) -> Peer {
        Peer {
            node_id: format!("id-{node}"),
            node: node.to_string(),
            role,
            match_index: m,
            applied_index: m,
            heartbeat_age_ms: hb,
        }
    }

    fn group(peers: Vec<Peer>) -> RaftGroup {
        RaftGroup {
            id: "data-1".into(),
            kind: GroupKind::Data,
            term: 5,
            commit_index: 100,
            applied_index: 97,
            config_index: 3,
            peers,
            snapshots: Vec::new(),
        }
    }

    #[test]
    fn a_missing_voter_degrades_while_a_quorum_answers() {
        let g = group(vec![
            peer("a", PeerRole::Leader, 100, None),
            peer("b", PeerRole::Voter, 100, Some(20)),
            peer("c", PeerRole::Voter, 40, None),
        ]);
        assert_eq!(g.quorum(), 2);
        assert_eq!(g.reachable_voters(), 2);
        assert_eq!(g.health(), GroupHealth::Degraded);
    }

    #[test]
    fn two_missing_voters_lose_the_quorum() {
        let g = group(vec![
            peer("a", PeerRole::Leader, 100, None),
            peer("b", PeerRole::Voter, 90, None),
            peer("c", PeerRole::Voter, 40, None),
        ]);
        assert_eq!(g.reachable_voters(), 1);
        assert_eq!(g.health(), GroupHealth::NoQuorum);
    }

    #[test]
    fn a_learner_never_counts_toward_the_voters() {
        let g = group(vec![
            peer("a", PeerRole::Leader, 100, None),
            peer("b", PeerRole::Voter, 100, Some(10)),
            peer("c", PeerRole::Learner, 0, Some(10)),
        ]);
        assert_eq!(g.voter_count(), 2);
        assert_eq!(g.learners().len(), 1);
        assert_eq!(g.health(), GroupHealth::Healthy);
    }

    #[test]
    fn index_lag_and_pending_entries_are_separate_measures() {
        let g = group(vec![
            peer("a", PeerRole::Leader, 100, None),
            peer("b", PeerRole::Voter, 90, Some(500)),
        ]);
        assert_eq!(g.pending_entries(), 3);
        assert_eq!(g.match_lag(&g.peers[1]), 10);
        assert_eq!(g.peers[1].state(), PeerState::Delayed);
    }

    #[test]
    fn only_a_reachable_voter_is_a_transfer_target() {
        let g = group(vec![
            peer("a", PeerRole::Leader, 100, None),
            peer("b", PeerRole::Voter, 100, Some(10)),
            peer("c", PeerRole::Voter, 40, None),
            peer("d", PeerRole::Learner, 0, Some(10)),
        ]);
        let names: Vec<_> = g
            .transfer_targets()
            .iter()
            .map(|p| p.node.clone())
            .collect();
        assert_eq!(names, vec!["b"]);
    }

    #[test]
    fn a_denied_preview_cannot_apply() {
        let p = TransferPreview {
            group: "g".into(),
            kind: GroupKind::Data,
            term: 1,
            from_id: "a".into(),
            from: "a".into(),
            to_id: "b".into(),
            to: "b".into(),
            target_lag: 0,
            voters_reachable: 3,
            voters_total: 3,
            effects: Vec::new(),
            warnings: Vec::new(),
            permission: PermissionCheck {
                required: "cluster.membership.write".into(),
                granted: false,
            },
            revision: 1,
        };
        assert!(!p.can_apply());
    }
}
