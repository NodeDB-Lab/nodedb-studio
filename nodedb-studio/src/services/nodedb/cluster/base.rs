//! Cluster reads and operations against the node Studio connected to.

use async_trait::async_trait;

use crate::models::cluster::activity::{ActivityLog, ActivityQuery};
use crate::models::cluster::monitor::{NodeTelemetry, Telemetry, TelemetryRange};
use crate::models::cluster::op::OpResult;
use crate::models::cluster::place::{MoveRequest, Placement, RebalancePlan};
use crate::models::cluster::raft::{RaftGroups, SnapshotInfo, TransferPreview};
use crate::models::cluster::{ClusterMembers, HostedGroup};
use crate::models::format::clock_second;
use crate::services::cluster_data::ClusterData;
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;
use crate::services::nodedb::cluster::assemble::{self, node_name};
use crate::services::nodedb::cluster::decode::{self, Seen};

/// A typed error for something NodeDB 0.6.2 cannot do.
fn unavailable(what: &str) -> StudioError {
    StudioError::Unsupported {
        what: format!("NodeDB 0.6.2 {what}"),
    }
}

/// A read of another node's own view, which one connection cannot make.
fn elsewhere(what: &str, node: &str, seen: &Seen) -> StudioError {
    StudioError::Unsupported {
        what: format!(
            "{what} of node {node} as node {node} sees them: Studio reads the cluster through node {}, the node it connected to. Connect to node {node} to read them.",
            seen.node_id
        ),
    }
}

/// Studio's clock: no cluster statement reports a time.
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

const SHOW_CLUSTER: &str = "SHOW CLUSTER";
const SHOW_NODES: &str = "SHOW NODES";
const SHOW_PEER_HEALTH: &str = "SHOW PEER HEALTH";
const SHOW_RAFT_GROUPS: &str = "SHOW RAFT GROUPS";
const SHOW_RANGES: &str = "SHOW RANGES";
const SHOW_MIGRATIONS: &str = "SHOW MIGRATIONS";
const SHOW_MEMORY: &str = "SHOW MEMORY";

impl NodeDbConnectionService {
    /// The node that answers this connection, and its hosted group count.
    async fn seen(&self) -> Result<Seen, StudioError> {
        decode::seen(&self.table(SHOW_CLUSTER).await?)
    }
}

#[async_trait(?Send)]
impl ClusterData for NodeDbConnectionService {
    async fn cluster_members(&self) -> Result<ClusterMembers, StudioError> {
        let seen = self.seen().await?;
        let nodes = decode::nodes(&self.table(SHOW_NODES).await?)?;
        let links = decode::links(&self.table(SHOW_PEER_HEALTH).await?)?;
        let at = format!("{} UTC", clock_second(now(), 0));
        Ok(assemble::members(&seen, &nodes, &links, at))
    }

    async fn hosted_groups(&self, node_id: &str) -> Result<Vec<HostedGroup>, StudioError> {
        let seen = self.seen().await?;
        if node_id != seen.node_id.to_string() {
            return Err(elsewhere("the Raft groups", node_id, &seen));
        }
        Ok(assemble::hosted(&decode::groups(
            &self.table(SHOW_RAFT_GROUPS).await?,
        )?))
    }

    async fn raft_groups(&self) -> Result<RaftGroups, StudioError> {
        let seen = self.seen().await?;
        let rows = decode::groups(&self.table(SHOW_RAFT_GROUPS).await?)?;
        let links = decode::links(&self.table(SHOW_PEER_HEALTH).await?)?;
        let mut groups = Vec::with_capacity(rows.len());
        for row in &rows {
            // The id is a number Studio parsed: safe in the statement text.
            let detail = self
                .table(&format!("SHOW RAFT GROUP {}", row.group_id))
                .await?;
            let members = decode::group_members(&detail)?;
            groups.push(assemble::raft_group(&seen, row, members, &links));
        }
        Ok(RaftGroups {
            observed_at: now(),
            revision: None,
            observer: assemble::observer(&seen),
            groups,
        })
    }

    async fn preview_leadership_transfer(
        &self,
        _group: &str,
        _target_id: &str,
    ) -> Result<TransferPreview, StudioError> {
        Err(unavailable(
            "has no statement that moves a group's leadership",
        ))
    }

    async fn transfer_leadership(
        &self,
        _group: &str,
        _target_id: &str,
        _revision: u64,
    ) -> Result<OpResult, StudioError> {
        Err(unavailable(
            "has no statement that moves a group's leadership",
        ))
    }

    async fn create_snapshot(&self, _group: &str) -> Result<SnapshotInfo, StudioError> {
        Err(unavailable("has no statement that snapshots a Raft group"))
    }

    /// `SHOW RANGES` also has `qps`, `p99_latency_ms` and `requests_total`.
    /// They count only the requests this node dispatched, and `qps` keeps
    /// the rate of the last one-second window that saw traffic, so it reads
    /// as current long after traffic stopped. No model field takes them.
    async fn placement(&self) -> Result<Placement, StudioError> {
        let seen = self.seen().await?;
        let nodes = decode::nodes(&self.table(SHOW_NODES).await?)?;
        let links = decode::links(&self.table(SHOW_PEER_HEALTH).await?)?;
        let ranges = decode::ranges(&self.table(SHOW_RANGES).await?)?;
        let moving = decode::migrations(&self.table(SHOW_MIGRATIONS).await?)?;
        Ok(assemble::placement(
            &seen,
            &nodes,
            &links,
            &ranges,
            &moving,
            now(),
        ))
    }

    /// `REBALANCE` takes no moves: it prints the server's own plan to even
    /// out group leaders and never runs it. That is not a preview of the
    /// staged replica moves, so Studio does not send it.
    async fn preview_rebalance(
        &self,
        _moves: &[MoveRequest],
    ) -> Result<RebalancePlan, StudioError> {
        Err(unavailable(
            "cannot preview replica moves: REBALANCE takes no moves and only prints a plan of its own",
        ))
    }

    async fn apply_rebalance(&self, _plan: &RebalancePlan) -> Result<OpResult, StudioError> {
        Err(unavailable("has no statement that moves a replica"))
    }

    /// The observer's engine budgets. No history and no CPU, process
    /// memory, disk or network exist on 0.6.2. `SHOW STATS` query counters
    /// stay 0 under load, so Studio does not read them.
    async fn telemetry(
        &self,
        node: Option<&str>,
        range: TelemetryRange,
    ) -> Result<Telemetry, StudioError> {
        let seen = self.seen().await?;
        if let Some(id) = node.filter(|id| *id != seen.node_id.to_string()) {
            return Err(elsewhere("the telemetry", id, &seen));
        }
        let budgets = decode::budgets(&self.table(SHOW_MEMORY).await?)?;
        let at = now();
        Ok(Telemetry {
            observed_at: at,
            range,
            scope: node.map(str::to_string),
            observer: assemble::observer(&seen),
            series: Vec::new(),
            nodes: vec![NodeTelemetry {
                node_id: seen.node_id.to_string(),
                node: node_name(seen.node_id),
                cpu_percent: None,
                memory_used_bytes: None,
                memory_total_bytes: None,
                disk_used_bytes: None,
                disk_total_bytes: None,
                net_in_mbs: None,
                net_out_mbs: None,
                observed_at: at,
            }],
            budgets,
        })
    }

    async fn cluster_activity(&self, _query: &ActivityQuery) -> Result<ActivityLog, StudioError> {
        Err(unavailable(
            "keeps no cluster event log: SHOW AUDIT LOG holds sign-in events only",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::cluster::raft::PeerRole;
    use crate::models::cluster::{GroupKind, Link, NodeState};
    use crate::services::nodedb::service::live;

    /// No session exists, so a method that answered would have sent
    /// nothing: every operation is refused before any statement.
    #[tokio::test]
    async fn operations_are_unsupported_and_send_nothing() {
        let svc = NodeDbConnectionService::default();
        let unsupported = |r: Result<(), StudioError>| {
            assert!(matches!(r, Err(StudioError::Unsupported { .. })), "{r:?}");
        };
        unsupported(svc.preview_leadership_transfer("1", "2").await.map(drop));
        unsupported(svc.transfer_leadership("1", "2", 1).await.map(drop));
        unsupported(svc.create_snapshot("1").await.map(drop));
        unsupported(svc.preview_rebalance(&[]).await.map(drop));
        let plan = RebalancePlan {
            moves: Vec::new(),
            warnings: Vec::new(),
            permission: crate::models::cluster::op::PermissionCheck {
                required: String::new(),
                granted: true,
            },
            revision: 1,
        };
        unsupported(svc.apply_rebalance(&plan).await.map(drop));
        unsupported(
            svc.cluster_activity(&ActivityQuery::default())
                .await
                .map(drop),
        );
        assert!(
            !StudioError::Unsupported {
                what: String::new()
            }
            .is_retriable()
        );
    }

    #[tokio::test]
    async fn reads_need_a_session() {
        let svc = NodeDbConnectionService::default();
        assert!(matches!(
            svc.cluster_members().await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.raft_groups().await,
            Err(StudioError::NotConnected)
        ));
    }

    /// The second node to compare with, from `NODEDB_PROBE_ADDR_2`.
    async fn live_second() -> Option<NodeDbConnectionService> {
        let addr = std::env::var("NODEDB_PROBE_ADDR_2").ok()?;
        let first = std::env::var("NODEDB_PROBE_ADDR").ok();
        // SAFETY: the live tests run one at a time (`live-server` group).
        unsafe { std::env::set_var("NODEDB_PROBE_ADDR", &addr) };
        let svc = live().await;
        if let Some(a) = first {
            unsafe { std::env::set_var("NODEDB_PROBE_ADDR", a) };
        }
        Some(svc)
    }

    /// Reads only. Every member, the observer named, node state and circuit
    /// apart, and the observer's own groups.
    #[tokio::test]
    #[ignore = "needs a live NodeDB 0.6.2 server: see `live`"]
    async fn live_cluster_members_and_groups_are_one_nodes_view() {
        let svc = live().await;
        let members = svc.cluster_members().await.expect("members");
        println!("{members:#?}");
        let me = &members.observer.node_id;
        assert!(members.members.iter().any(|m| &m.id == me));
        for m in &members.members {
            assert!(!matches!(m.state, NodeState::Unknown(_)), "{m:?}");
            if &m.id == me {
                assert_eq!(m.link, Link::Observer);
                assert!(m.hosted_groups.is_some());
            } else {
                assert!(matches!(m.link, Link::Circuit { .. }), "{m:?}");
                assert_eq!(m.hosted_groups, None);
            }
        }
        let hosted = svc.hosted_groups(me).await.expect("own groups");
        assert!(hosted.iter().any(|g| g.kind == GroupKind::Metadata));
        assert!(hosted.iter().all(|g| g.state.is_none()));
        if let Some(other) = members.members.iter().find(|m| &m.id != me) {
            assert!(matches!(
                svc.hosted_groups(&other.id).await,
                Err(StudioError::Unsupported { .. })
            ));
        }

        let groups = svc.raft_groups().await.expect("groups");
        assert_eq!(&groups.observer.node_id, me);
        assert_eq!(groups.groups.len(), hosted.len());
        let seq = groups
            .groups
            .iter()
            .find(|g| g.kind == GroupKind::Sequencer)
            .expect("the sequencer group");
        assert_eq!(seq.id, "18446744069414584321");
        for g in groups
            .groups
            .iter()
            .filter(|g| g.kind != GroupKind::Sequencer)
        {
            let peers = g
                .peers
                .as_ref()
                .expect("a data or metadata group lists members");
            assert_eq!(peers.len() as u32, g.voter_count, "group {}", g.id);
            assert!(peers.iter().all(|p| p.progress.is_none()));
            assert!(peers.iter().filter(|p| p.role == PeerRole::Leader).count() <= 1);
        }
    }

    /// Reads only: placement over every vShard, and the observer's memory
    /// budgets. A second node, when `NODEDB_PROBE_ADDR_2` names one, sees
    /// the same members from its own place.
    #[tokio::test]
    #[ignore = "needs a live NodeDB 0.6.2 server: see `live`"]
    async fn live_cluster_placement_and_telemetry_are_point_in_time() {
        let svc = live().await;
        let p = svc.placement().await.expect("placement");
        assert_eq!(p.vshards.len(), 1024);
        assert!(
            p.vshards
                .iter()
                .all(|v| v.size_bytes.is_none() && v.source().is_none())
        );
        assert!(p.nodes.iter().all(|n| n.disk_used_bytes.is_none()));
        assert!(!p.quorums.is_empty());
        let me = p.observer.node_id.clone();

        let t = svc
            .telemetry(None, TelemetryRange::Hour1)
            .await
            .expect("telemetry");
        assert!(t.series.is_empty());
        assert_eq!(t.nodes.len(), 1);
        assert_eq!(t.nodes[0].cpu_percent, None);
        assert!(
            t.budgets
                .iter()
                .any(|b| b.engine == "Vector" && b.limit_bytes > 0)
        );
        assert!(t.budgets.iter().all(|b| b.reserved_bytes <= b.limit_bytes));

        let Some(other) = live_second().await else {
            println!("NODEDB_PROBE_ADDR_2 unset: no second view to compare");
            return;
        };
        let theirs = other.cluster_members().await.expect("members there");
        assert_ne!(
            theirs.observer.node_id, me,
            "the second address is the same node"
        );
        let ids = |m: &ClusterMembers| m.members.iter().map(|x| x.id.clone()).collect::<Vec<_>>();
        let ours = svc.cluster_members().await.expect("members here");
        assert_eq!(ids(&ours), ids(&theirs));
        assert!(matches!(
            svc.telemetry(Some(&theirs.observer.node_id), TelemetryRange::Hour1)
                .await,
            Err(StudioError::Unsupported { .. })
        ));
    }
}
