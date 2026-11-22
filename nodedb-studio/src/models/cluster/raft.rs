//! Raft groups as one node of an Origin cluster reports them: one group, its
//! peers, and its log positions. Leadership, term, quorum, and every index
//! belong to a group, and every value is the observer's view of it. Index
//! lag counts entries and heartbeat age counts milliseconds: they are
//! separate measures with separate fields and separate columns.

use serde::{Deserialize, Serialize};

use crate::models::cluster::op::{PermissionCheck, quorum_of};
use crate::models::cluster::{GroupKind, Link, Observer};
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

/// What a group's leader records of one peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerProgress {
    /// The highest log index the leader knows this peer holds.
    pub match_index: u64,
    /// The highest index this peer has applied to its state machine.
    pub applied_index: u64,
    /// Milliseconds since the last heartbeat answer. `None` is no answer.
    pub heartbeat_age_ms: Option<u64>,
}

/// One member of a group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Peer {
    pub node_id: String,
    /// Display name, e.g. `node-4`.
    pub node: String,
    pub role: PeerRole,
    /// The leader's record of this peer. `None`: not reported. NodeDB 0.6.2
    /// has no statement for the leader's progress of each peer.
    pub progress: Option<PeerProgress>,
    /// How the observer reaches this peer.
    pub link: Link,
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

/// One Raft group, as the observer holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaftGroup {
    /// Group id, e.g. `data-12`, or `2` on NodeDB.
    pub id: String,
    pub kind: GroupKind,
    /// Display name of the leader the observer knows. `None`: it knows none.
    pub leader: Option<String>,
    /// The term, commit index and applied index the observer holds.
    pub term: u64,
    pub commit_index: u64,
    pub applied_index: u64,
    /// The log index of the group's current membership configuration.
    /// `None`: not reported.
    pub config_index: Option<u64>,
    /// Voters, as the observer's Raft counts them.
    pub voter_count: u32,
    /// The members. `None`: not reported, as for the NodeDB sequencer
    /// group, which has no routing entry.
    pub peers: Option<Vec<Peer>>,
    /// Whether `peers` lists the learners too. NodeDB lists voters only.
    pub learners_reported: bool,
    /// `None`: not reported. NodeDB 0.6.2 lists no snapshots.
    pub snapshots: Option<Vec<SnapshotInfo>>,
}

impl RaftGroup {
    /// The members, or none when they are not reported.
    pub fn peers(&self) -> &[Peer] {
        self.peers.as_deref().unwrap_or(&[])
    }

    pub fn leader(&self) -> Option<&Peer> {
        self.peers().iter().find(|p| p.role == PeerRole::Leader)
    }

    pub fn leader_name(&self) -> &str {
        self.leader.as_deref().unwrap_or("none")
    }

    pub fn voters(&self) -> Vec<&Peer> {
        self.peers().iter().filter(|p| p.role.votes()).collect()
    }

    pub fn learners(&self) -> Vec<&Peer> {
        self.peers().iter().filter(|p| !p.role.votes()).collect()
    }

    pub fn quorum(&self) -> u32 {
        quorum_of(self.voter_count)
    }

    /// Voters the observer reaches. `None` when the members are not
    /// reported, the observer cannot say for one of them, or the listed
    /// voters disagree with `voter_count`. On NodeDB the list comes from
    /// the routing table and the count from Raft: during a membership
    /// change they differ, and either quorum would be a guess.
    pub fn reachable_voters(&self) -> Option<u32> {
        self.peers.as_ref()?;
        if self.voters().len() != self.voter_count as usize {
            return None;
        }
        self.voters()
            .iter()
            .map(|p| p.link.reached())
            .try_fold(0, |n, r| r.map(|up| n + u32::from(up)))
    }

    /// The group's health from the observer's links. `None` when the
    /// reachable voters are not known.
    pub fn health(&self) -> Option<GroupHealth> {
        let up = self.reachable_voters()?;
        Some(if up < self.quorum() {
            GroupHealth::NoQuorum
        } else if up < self.voter_count {
            GroupHealth::Degraded
        } else {
            GroupHealth::Healthy
        })
    }

    /// Committed entries the observer has not applied yet.
    pub fn pending_entries(&self) -> u64 {
        self.commit_index.saturating_sub(self.applied_index)
    }

    /// Entries a peer's match index trails the commit index by. `None`
    /// when the leader's record of the peer is not reported.
    pub fn match_lag(&self, peer: &Peer) -> Option<u64> {
        peer.progress
            .map(|p| self.commit_index.saturating_sub(p.match_index))
    }

    pub fn peer(&self, node_id: &str) -> Option<&Peer> {
        self.peers().iter().find(|p| p.node_id == node_id)
    }

    /// Voters other than the leader that the observer reaches: the members
    /// leadership can move to.
    pub fn transfer_targets(&self) -> Vec<&Peer> {
        self.peers()
            .iter()
            .filter(|p| p.role == PeerRole::Voter && p.link.reached() == Some(true))
            .collect()
    }
}

/// Every group the observer hosts, whose view it is, the time Studio read
/// it, and the revision a write must name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaftGroups {
    pub observed_at: i64,
    /// `None`: the server numbers no revision.
    pub revision: Option<u64>,
    pub observer: Observer,
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
            .find(|g| g.health().is_some_and(|h| h != GroupHealth::Healthy))
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
    use crate::models::cluster::Circuit;

    fn link(up: bool) -> Link {
        Link::Circuit {
            state: if up { Circuit::Closed } else { Circuit::Open },
            failures: if up { 0 } else { 5 },
        }
    }

    fn peer(node: &str, role: PeerRole, m: u64, up: bool) -> Peer {
        Peer {
            node_id: format!("id-{node}"),
            node: node.to_string(),
            role,
            progress: Some(PeerProgress {
                match_index: m,
                applied_index: m,
                heartbeat_age_ms: up.then_some(20),
            }),
            link: link(up),
        }
    }

    fn group(peers: Vec<Peer>) -> RaftGroup {
        RaftGroup {
            id: "data-1".into(),
            kind: GroupKind::Data,
            leader: peers
                .iter()
                .find(|p| p.role == PeerRole::Leader)
                .map(|p| p.node.clone()),
            term: 5,
            commit_index: 100,
            applied_index: 97,
            config_index: Some(3),
            voter_count: peers.iter().filter(|p| p.role.votes()).count() as u32,
            peers: Some(peers),
            learners_reported: true,
            snapshots: Some(Vec::new()),
        }
    }

    #[test]
    fn a_missing_voter_degrades_while_a_quorum_answers() {
        let g = group(vec![
            peer("a", PeerRole::Leader, 100, true),
            peer("b", PeerRole::Voter, 100, true),
            peer("c", PeerRole::Voter, 40, false),
        ]);
        assert_eq!(g.quorum(), 2);
        assert_eq!(g.reachable_voters(), Some(2));
        assert_eq!(g.health(), Some(GroupHealth::Degraded));
    }

    #[test]
    fn two_missing_voters_lose_the_quorum() {
        let g = group(vec![
            peer("a", PeerRole::Leader, 100, true),
            peer("b", PeerRole::Voter, 90, false),
            peer("c", PeerRole::Voter, 40, false),
        ]);
        assert_eq!(g.reachable_voters(), Some(1));
        assert_eq!(g.health(), Some(GroupHealth::NoQuorum));
    }

    #[test]
    fn a_learner_never_counts_toward_the_voters() {
        let g = group(vec![
            peer("a", PeerRole::Leader, 100, true),
            peer("b", PeerRole::Voter, 100, true),
            peer("c", PeerRole::Learner, 0, true),
        ]);
        assert_eq!(g.voter_count, 2);
        assert_eq!(g.learners().len(), 1);
        assert_eq!(g.health(), Some(GroupHealth::Healthy));
    }

    #[test]
    fn index_lag_and_pending_entries_are_separate_measures() {
        let g = group(vec![
            peer("a", PeerRole::Leader, 100, true),
            peer("b", PeerRole::Voter, 90, true),
        ]);
        assert_eq!(g.pending_entries(), 3);
        assert_eq!(g.match_lag(&g.peers()[1]), Some(10));
    }

    /// The server's view of one node cannot say how a group does when it
    /// does not list the members or cannot name a circuit.
    #[test]
    fn health_is_not_reported_without_every_voter_link() {
        let mut g = group(vec![
            peer("a", PeerRole::Leader, 100, true),
            peer("b", PeerRole::Voter, 100, true),
        ]);
        if let Some(p) = g.peers.as_mut() {
            p[1].link = Link::NotReported;
            p[1].progress = None;
        }
        assert_eq!(g.reachable_voters(), None);
        assert_eq!(g.health(), None);
        assert_eq!(g.match_lag(&g.peers()[1]), None);
        g.peers = None;
        assert_eq!(g.health(), None);
        assert_eq!(g.quorum(), 2);
    }

    /// During a membership change the routing members and the Raft voter
    /// count differ. Counting one against the other's quorum would give a
    /// false Degraded or a false Healthy.
    #[test]
    fn health_is_not_reported_when_members_and_voter_count_disagree() {
        let mut g = group(vec![
            peer("a", PeerRole::Leader, 100, true),
            peer("b", PeerRole::Voter, 100, true),
            peer("c", PeerRole::Voter, 100, true),
        ]);
        assert_eq!(g.health(), Some(GroupHealth::Healthy));
        g.voter_count = 4;
        assert_eq!(g.reachable_voters(), None);
        assert_eq!(g.health(), None);
        g.voter_count = 2;
        assert_eq!(g.health(), None);
    }

    #[test]
    fn only_a_reachable_voter_is_a_transfer_target() {
        let g = group(vec![
            peer("a", PeerRole::Leader, 100, true),
            peer("b", PeerRole::Voter, 100, true),
            peer("c", PeerRole::Voter, 40, false),
            peer("d", PeerRole::Learner, 0, true),
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
