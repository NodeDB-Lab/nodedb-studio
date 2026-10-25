//! The cluster the mock serves, with the writes applied to it. Groups,
//! placement, and activity share one store, so a leadership transfer shows
//! in the Raft group, in the member's hosted groups, and in the activity log.
//! Every write names the revision it read and is refused when it changed.

use crate::data::mock::cluster::activity::{event, seeded_events};
use crate::data::mock::cluster::members::{OBSERVED, cluster_members, node_name};
use crate::data::mock::cluster::placement::placement;
use crate::data::mock::cluster::raft::{hosted_groups, seeded_groups};
use crate::models::cluster::HostedGroup;
use crate::models::cluster_activity::{
    ActivityLog, ActivityQuery, ClusterEvent, EventKind, Severity,
};
use crate::models::cluster_op::{Impact, OpResult, PermissionCheck};
use crate::models::cluster_place::{
    MoveRequest, Movement, Placement, PlannedMove, RebalancePlan, size_text,
};
use crate::models::cluster_raft::{
    Peer, PeerRole, RaftGroup, RaftGroups, SnapshotInfo, TransferPreview,
};
use crate::services::error::StudioError;

/// The privilege a membership change needs.
const MEMBERSHIP: &str = "cluster.membership.write";
const ADMIN: &str = "admin";

#[derive(Debug, Clone)]
pub struct ClusterStore {
    revision: u64,
    clock: i64,
    groups: Vec<RaftGroup>,
    moving: Vec<(String, Movement)>,
    events: Vec<ClusterEvent>,
    next_operation: u32,
    next_event: u32,
    granted: bool,
}

fn missing(kind: &'static str, id: &str) -> StudioError {
    StudioError::UnknownObject {
        kind,
        id: id.to_string(),
    }
}

impl ClusterStore {
    pub fn seeded() -> Self {
        Self {
            revision: 1,
            clock: OBSERVED,
            groups: seeded_groups(),
            moving: Vec::new(),
            events: seeded_events(),
            next_operation: 73,
            next_event: 98148,
            granted: true,
        }
    }

    /// A store whose principal lacks the membership privilege.
    #[cfg(test)]
    pub fn without_permission() -> Self {
        Self {
            granted: false,
            ..Self::seeded()
        }
    }

    fn permission(&self) -> PermissionCheck {
        PermissionCheck {
            required: MEMBERSHIP.to_string(),
            granted: self.granted,
        }
    }

    fn stale(&self, object: String, expected: u64) -> Option<StudioError> {
        (expected != self.revision).then_some(StudioError::StaleRevision {
            object,
            expected,
            stored: self.revision,
        })
    }

    pub fn raft_groups(&self) -> RaftGroups {
        RaftGroups {
            observed_at: self.clock,
            revision: self.revision,
            groups: self.groups.clone(),
        }
    }

    pub fn hosted(&self, node_id: &str) -> Vec<HostedGroup> {
        hosted_groups(&self.groups, node_id)
    }

    pub fn placement(&self) -> Placement {
        placement(&self.groups, &self.moving, self.revision)
    }

    pub fn activity(&self, q: &ActivityQuery) -> ActivityLog {
        let floor = self.clock - q.range.seconds();
        let events = self
            .events
            .iter()
            .filter(|e| e.observed_at >= floor)
            .filter(|e| q.severity.is_none_or(|s| e.severity == s))
            .filter(|e| q.node.as_deref().is_none_or(|n| e.node_id == n))
            .filter(|e| {
                q.group
                    .as_deref()
                    .is_none_or(|g| e.group.as_deref() == Some(g))
            })
            .cloned()
            .collect();
        ActivityLog {
            observed_at: self.clock,
            events,
        }
    }

    fn group(&self, id: &str) -> Result<&RaftGroup, StudioError> {
        self.groups
            .iter()
            .find(|g| g.id == id)
            .ok_or_else(|| missing("Raft group", id))
    }

    fn operation(&mut self) -> String {
        let id = format!("op_{}", self.next_operation);
        self.next_operation += 1;
        id
    }

    /// Record an event at the next tick of the clock.
    fn record(&mut self, mut e: ClusterEvent) {
        self.clock += 14;
        e.id = format!("evt_{}", self.next_event);
        self.next_event += 1;
        e.observed_at = self.clock;
        e.event_time = self.clock - 1;
        self.events.insert(0, e);
    }

    pub fn preview_transfer(
        &self,
        group: &str,
        target: &str,
    ) -> Result<TransferPreview, StudioError> {
        let g = self.group(group)?;
        let leader = g.leader().ok_or_else(|| StudioError::Rejected {
            reason: format!("{group} has no leader to hand over"),
        })?;
        let to = g.peer(target).ok_or_else(|| missing("peer", target))?;
        if !g.transfer_targets().iter().any(|p| p.node_id == to.node_id) {
            return Err(StudioError::Rejected {
                reason: format!(
                    "{} is not a reachable voter of {group}: leadership moves only to one",
                    to.node
                ),
            });
        }
        let lag = g.match_lag(to);
        let mut effects = vec![
            format!(
                "{} takes leadership of {group} in term {}.",
                to.node,
                g.term + 1
            ),
            format!("{} becomes a voter of {group}.", leader.node),
            "Other groups keep their leaders.".to_string(),
        ];
        let mut warnings = Vec::new();
        if lag > 0 {
            effects.push(format!(
                "{} must first receive the {lag} entries it lacks: the transfer waits for that.",
                to.node
            ));
        }
        if g.reachable_voters() < g.voter_count() {
            warnings.push(format!(
                "{group} has {} of {} voters reachable. A second loss during the transfer stops it from committing.",
                g.reachable_voters(),
                g.voter_count()
            ));
        }
        Ok(TransferPreview {
            group: g.id.clone(),
            kind: g.kind,
            term: g.term,
            from_id: leader.node_id.clone(),
            from: leader.node.clone(),
            to_id: to.node_id.clone(),
            to: to.node.clone(),
            target_lag: lag,
            voters_reachable: g.reachable_voters(),
            voters_total: g.voter_count(),
            effects,
            warnings,
            permission: self.permission(),
            revision: self.revision,
        })
    }

    pub fn transfer(
        &mut self,
        group: &str,
        target: &str,
        revision: u64,
    ) -> Result<OpResult, StudioError> {
        if let Some(e) = self.stale(format!("Raft group {group}"), revision) {
            return Err(e);
        }
        if !self.granted {
            return Err(StudioError::PermissionDenied {
                privilege: MEMBERSHIP,
                collection: "cluster".to_string(),
            });
        }
        let preview = self.preview_transfer(group, target)?;
        let op = self.operation();
        if let Some(g) = self.groups.iter_mut().find(|g| g.id == group) {
            g.term += 1;
            for p in &mut g.peers {
                if p.node_id == preview.from_id {
                    p.role = PeerRole::Voter;
                } else if p.node_id == preview.to_id {
                    p.role = PeerRole::Leader;
                    p.heartbeat_age_ms = Some(0);
                }
            }
        }
        let g = self.group(group)?.clone();
        let mut e = event(
            0,
            0,
            Severity::Info,
            EventKind::LeadershipTransferred,
            Some(group),
            node_number(&preview.to_id),
            node_number(&preview.to_id),
        );
        e.term = Some(g.term);
        e.previous_leader = Some(preview.from.clone());
        e.leader = Some(preview.to.clone());
        e.voters = g.voters().iter().map(|p| p.node.clone()).collect();
        e.correlation = Some(op.clone());
        e.actor = Some(ADMIN.to_string());
        self.record(e);
        self.revision += 1;
        Ok(OpResult {
            operation: op,
            summary: format!(
                "Moved leadership of {group} from {} to {}.",
                preview.from, preview.to
            ),
        })
    }

    pub fn create_snapshot(&mut self, group: &str) -> Result<SnapshotInfo, StudioError> {
        if !self.granted {
            return Err(StudioError::PermissionDenied {
                privilege: MEMBERSHIP,
                collection: "cluster".to_string(),
            });
        }
        let g = self.group(group)?;
        let leader = g.leader().map(|p| p.node.clone()).unwrap_or_default();
        let n = g.snapshots.len() as u32 + 1;
        let snap = SnapshotInfo {
            id: format!("snap_{group}_{n}"),
            index: g.applied_index,
            taken_at: self.clock + 14,
            size_bytes: g
                .snapshots
                .first()
                .map(|s| s.size_bytes + 40 * 1024 * 1024)
                .unwrap_or(64 * 1024 * 1024),
            node: leader.clone(),
        };
        let op = self.operation();
        if let Some(g) = self.groups.iter_mut().find(|g| g.id == group) {
            g.snapshots.insert(0, snap.clone());
        }
        let mut e = event(
            0,
            0,
            Severity::Info,
            EventKind::SnapshotCreated,
            Some(group),
            node_number_by_name(&leader),
            node_number_by_name(&leader),
        );
        e.correlation = Some(op);
        e.actor = Some(ADMIN.to_string());
        self.record(e);
        self.revision += 1;
        Ok(snap)
    }

    pub fn preview_rebalance(&self, moves: &[MoveRequest]) -> Result<RebalancePlan, StudioError> {
        if moves.is_empty() {
            return Err(StudioError::Rejected {
                reason: "the plan has no move".to_string(),
            });
        }
        let placement = self.placement();
        let members = cluster_members().members;
        let mut planned: Vec<PlannedMove> = Vec::new();
        let mut warnings = Vec::new();
        for m in moves {
            let v = placement
                .vshard(m.vshard)
                .ok_or_else(|| missing("vShard", &m.vshard.to_string()))?;
            let g = self.group(&v.group)?;
            if planned.iter().any(|p| p.group == g.id) {
                return Err(StudioError::Rejected {
                    reason: format!("two moves change {}: stage one per group", g.id),
                });
            }
            let from = g
                .peer(&m.from_id)
                .ok_or_else(|| missing("replica", &m.from_id))?;
            if g.peer(&m.to_id).is_some() {
                return Err(StudioError::Rejected {
                    reason: format!(
                        "{} already holds a replica of {}",
                        node_name_of(&m.to_id),
                        g.id
                    ),
                });
            }
            let to = members
                .iter()
                .find(|n| n.id == m.to_id)
                .ok_or_else(|| missing("node", &m.to_id))?;
            if !to.state.reachable() {
                return Err(StudioError::Rejected {
                    reason: format!("{} does not answer: it cannot receive a replica", to.name),
                });
            }
            // The replica's place among the group's members, by node id.
            let mut ids: Vec<&String> = g.peers.iter().map(|p| &p.node_id).collect();
            ids.sort();
            let slot = ids.iter().position(|id| **id == from.node_id).unwrap_or(0) + 1;
            planned.push(PlannedMove {
                vshard: m.vshard,
                group: g.id.clone(),
                replica: format!("Replica {slot}"),
                source: from.node.clone(),
                destination: to.name.clone(),
                estimated_bytes: placement.group_bytes(&g.id),
                impact: Impact::Unknown,
                vshards: placement.group_vshards(&g.id),
            });
            if g.reachable_voters() < g.voter_count() && from.state().reachable() {
                warnings.push(format!(
                    "{} has {} of {} voters reachable. Removing a reachable replica leaves less margin.",
                    g.id,
                    g.reachable_voters(),
                    g.voter_count()
                ));
            }
        }
        Ok(RebalancePlan {
            moves: planned,
            warnings,
            permission: self.permission(),
            revision: self.revision,
        })
    }

    pub fn apply_rebalance(&mut self, plan: &RebalancePlan) -> Result<OpResult, StudioError> {
        if let Some(e) = self.stale("the placement plan".to_string(), plan.revision) {
            return Err(e);
        }
        if !self.granted {
            return Err(StudioError::PermissionDenied {
                privilege: MEMBERSHIP,
                collection: "cluster".to_string(),
            });
        }
        let op = self.operation();
        let mut bytes = 0;
        for m in &plan.moves {
            self.group(&m.group)?;
            let dest = cluster_members()
                .members
                .into_iter()
                .find(|n| n.name == m.destination)
                .ok_or_else(|| missing("node", &m.destination))?;
            bytes += m.estimated_bytes;
            if let Some(g) = self.groups.iter_mut().find(|g| g.id == m.group) {
                g.peers.push(Peer {
                    node_id: dest.id.clone(),
                    node: dest.name.clone(),
                    role: PeerRole::Learner,
                    match_index: 0,
                    applied_index: 0,
                    heartbeat_age_ms: Some(24),
                });
            }
            self.moving.push((
                m.group.clone(),
                Movement {
                    operation: op.clone(),
                    from: m.source.clone(),
                    to: m.destination.clone(),
                },
            ));
            let mut e = event(
                0,
                0,
                Severity::Info,
                EventKind::PlacementJobStarted,
                Some(&m.group),
                node_number(&dest.id),
                node_number(&dest.id),
            );
            e.correlation = Some(op.clone());
            e.actor = Some(ADMIN.to_string());
            self.record(e);
        }
        self.revision += 1;
        Ok(OpResult {
            operation: op,
            summary: format!(
                "Started {} replica move{}, about {} to copy.",
                plan.moves.len(),
                if plan.moves.len() == 1 { "" } else { "s" },
                size_text(bytes)
            ),
        })
    }

    /// The vShards, for tests.
    #[cfg(test)]
    pub fn vshard_numbers(&self) -> Vec<u32> {
        crate::data::mock::cluster::placement::vshards(&self.groups, &self.moving)
            .iter()
            .map(|v| v.number)
            .collect()
    }
}

fn node_number(id: &str) -> u32 {
    id.trim_start_matches("nd_").parse().unwrap_or(0)
}

fn node_number_by_name(name: &str) -> u32 {
    name.trim_start_matches("node-").parse().unwrap_or(0)
}

fn node_name_of(id: &str) -> String {
    node_name(node_number(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::mock::cluster::members::node_id;

    fn nd(n: u32) -> String {
        node_id(n)
    }

    #[test]
    fn a_transfer_moves_leadership_and_logs_it() {
        let mut s = ClusterStore::seeded();
        let before = s.raft_groups();
        let out = s
            .transfer("data-11", &nd(2), before.revision)
            .expect("transfer");
        assert_eq!(out.operation, "op_73");
        let g = s.raft_groups();
        let g11 = g.group("data-11").expect("group");
        assert_eq!(g11.leader_name(), "node-2");
        assert_eq!(g11.term, 88);
        // Other groups keep their leaders.
        assert_eq!(g.group("data-12").expect("group").leader_name(), "node-2");
        assert_eq!(
            g.group("metadata-0").expect("group").leader_name(),
            "node-1"
        );
        let log = s.activity(&ActivityQuery::default());
        assert_eq!(log.events[0].kind, EventKind::LeadershipTransferred);
        assert_eq!(log.events[0].correlation.as_deref(), Some("op_73"));
        assert_eq!(
            s.hosted(&nd(2))[1].role_here,
            crate::models::cluster::GroupRole::Leader
        );
    }

    #[test]
    fn a_stale_preview_never_applies() {
        let mut s = ClusterStore::seeded();
        let first = s.raft_groups().revision;
        s.transfer("data-11", &nd(2), first).expect("first");
        let err = s.transfer("data-11", &nd(1), first).expect_err("stale");
        assert!(matches!(err, StudioError::StaleRevision { .. }), "{err}");
    }

    #[test]
    fn leadership_moves_only_to_a_reachable_voter() {
        let s = ClusterStore::seeded();
        // node-5 does not answer. node-1 is not a voter of data-12.
        assert!(s.preview_transfer("data-12", &nd(5)).is_err());
        assert!(s.preview_transfer("data-12", &nd(1)).is_err());
        assert!(s.preview_transfer("data-12", &nd(4)).is_ok());
        assert!(s.preview_transfer("data-99", &nd(4)).is_err());
    }

    #[test]
    fn a_lagging_target_and_a_missing_voter_are_named() {
        let s = ClusterStore::seeded();
        let p = s.preview_transfer("data-12", &nd(4)).expect("preview");
        assert_eq!(p.target_lag, 880);
        assert!(p.effects.iter().any(|e| e.contains("880 entries")));
        assert_eq!((p.voters_reachable, p.voters_total), (2, 3));
        assert_eq!(p.warnings.len(), 1);
    }

    #[test]
    fn without_the_privilege_nothing_changes() {
        let mut s = ClusterStore::without_permission();
        let p = s.preview_transfer("data-11", &nd(2)).expect("preview");
        assert!(!p.can_apply());
        let err = s
            .transfer("data-11", &nd(2), p.revision)
            .expect_err("denied");
        assert!(matches!(err, StudioError::PermissionDenied { .. }), "{err}");
        assert_eq!(s.raft_groups().group("data-11").expect("g").term, 87);
    }

    #[test]
    fn a_rebalance_adds_a_learner_and_marks_every_vshard_of_the_group() {
        let mut s = ClusterStore::seeded();
        let plan = s
            .preview_rebalance(&[MoveRequest {
                vshard: 17,
                from_id: nd(5),
                to_id: nd(1),
            }])
            .expect("plan");
        let m = &plan.moves[0];
        assert_eq!(m.group, "data-12");
        assert_eq!(m.vshards, vec![14, 17, 21]);
        assert_eq!(m.impact, Impact::Unknown);
        assert_eq!(m.estimated_bytes, 51 * crate::models::cluster_place::GIB);
        let out = s.apply_rebalance(&plan).expect("applied");
        assert_eq!(out.operation, "op_73");
        let p = s.placement();
        for n in [14, 17, 21] {
            assert!(p.vshard(n).expect("shard").movement.is_some());
        }
        assert!(p.vshard(3).expect("shard").movement.is_none());
        let g = s.raft_groups();
        let g12 = g.group("data-12").expect("group");
        assert_eq!(g12.learners().len(), 1);
        assert_eq!(g12.voter_count(), 3);
        assert!(s.vshard_numbers().contains(&17));
    }

    #[test]
    fn a_rebalance_refuses_bad_moves() {
        let s = ClusterStore::seeded();
        let bad = |vshard, from, to| {
            s.preview_rebalance(&[MoveRequest {
                vshard,
                from_id: nd(from),
                to_id: nd(to),
            }])
        };
        // node-4 already holds data-12.
        assert!(bad(17, 5, 4).is_err());
        // node-3 is not a replica of data-12.
        assert!(bad(17, 3, 1).is_err());
        // node-5 does not answer: it cannot receive.
        assert!(bad(3, 4, 5).is_err());
        assert!(bad(999, 5, 1).is_err());
        assert!(s.preview_rebalance(&[]).is_err());
        // Two moves in one group.
        let two = s.preview_rebalance(&[
            MoveRequest {
                vshard: 17,
                from_id: nd(5),
                to_id: nd(1),
            },
            MoveRequest {
                vshard: 21,
                from_id: nd(4),
                to_id: nd(3),
            },
        ]);
        assert!(two.is_err());
    }

    #[test]
    fn a_snapshot_joins_the_group_and_the_log() {
        let mut s = ClusterStore::seeded();
        let snap = s.create_snapshot("data-13").expect("snapshot");
        assert_eq!(snap.index, 733_950);
        assert_eq!(
            s.raft_groups().group("data-13").expect("g").snapshots[0].id,
            snap.id
        );
        assert_eq!(
            s.activity(&ActivityQuery::default()).events[0].kind,
            EventKind::SnapshotCreated
        );
    }

    #[test]
    fn activity_filters_by_range_severity_node_and_group() {
        let s = ClusterStore::seeded();
        let all = s.activity(&ActivityQuery::default());
        assert!(all.events.len() >= 10);
        let warn = s.activity(&ActivityQuery {
            severity: Some(Severity::Warning),
            ..ActivityQuery::default()
        });
        assert!(warn.events.iter().all(|e| e.severity == Severity::Warning));
        let node = s.activity(&ActivityQuery {
            node: Some(nd(5)),
            ..ActivityQuery::default()
        });
        assert!(node.events.iter().all(|e| e.node_id == nd(5)));
        let group = s.activity(&ActivityQuery {
            group: Some("data-12".to_string()),
            ..ActivityQuery::default()
        });
        assert!(
            group
                .events
                .iter()
                .all(|e| e.group.as_deref() == Some("data-12"))
        );
        let hour = s.activity(&ActivityQuery {
            range: crate::models::cluster_activity::ActivityRange::Hour1,
            ..ActivityQuery::default()
        });
        assert!(hour.events.len() < all.events.len());
        let week = s.activity(&ActivityQuery {
            range: crate::models::cluster_activity::ActivityRange::Days7,
            ..ActivityQuery::default()
        });
        assert!(week.events.len() > all.events.len());
    }

    #[test]
    fn event_ids_are_unique() {
        let s = ClusterStore::seeded();
        let mut ids: Vec<_> = s.events.iter().map(|e| e.id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), s.events.len());
    }

    #[test]
    fn members_and_groups_agree_on_what_each_node_hosts() {
        let s = ClusterStore::seeded();
        for m in cluster_members().members {
            assert_eq!(
                s.hosted(&m.id).len() as u32,
                m.hosted_groups,
                "{} hosted_groups",
                m.id
            );
        }
    }
}
