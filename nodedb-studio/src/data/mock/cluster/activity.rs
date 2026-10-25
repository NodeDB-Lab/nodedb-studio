//! Cluster activity fixtures: membership and leadership changes, and the
//! administrative operations of the last day, newest first. Times are the
//! server's clock around 2026-06-13 13:40 UTC.

use crate::data::mock::cluster::members::{node_id, node_name};
use crate::models::cluster_activity::{ClusterEvent, EventKind, Severity};
use crate::models::format::epoch;

/// One event. `observed` is when the server saw it, `happened` when the
/// source node says it did: a second earlier.
pub fn event(
    id: u32,
    at: i64,
    severity: Severity,
    kind: EventKind,
    group: Option<&str>,
    node: u32,
    source: u32,
) -> ClusterEvent {
    ClusterEvent {
        id: format!("evt_{id}"),
        observed_at: at,
        event_time: at - 1,
        severity,
        kind,
        group: group.map(str::to_string),
        node_id: node_id(node),
        node: node_name(node),
        term: None,
        previous_leader: None,
        leader: None,
        voters: Vec::new(),
        source: node_name(source),
        correlation: None,
        actor: None,
    }
}

fn voters(ns: &[u32]) -> Vec<String> {
    ns.iter().map(|n| node_name(*n)).collect()
}

fn at(h: u32, m: u32, s: u32) -> i64 {
    epoch(2026, 6, 13, h, m, s)
}

pub fn seeded_events() -> Vec<ClusterEvent> {
    use EventKind as K;
    use Severity as S;
    let mut out = Vec::new();

    let mut e = event(
        98147,
        at(13, 39, 41),
        S::Warning,
        K::HeartbeatDelayed,
        Some("data-12"),
        4,
        2,
    );
    e.term = Some(91);
    e.leader = Some(node_name(2));
    e.voters = voters(&[2, 4, 5]);
    out.push(e);

    let mut e = event(
        98146,
        at(13, 38, 20),
        S::Info,
        K::SnapshotInstalled,
        Some("data-11"),
        4,
        1,
    );
    e.term = Some(87);
    e.leader = Some(node_name(1));
    e.voters = voters(&[1, 2, 4]);
    out.push(e);

    let mut e = event(
        98145,
        at(13, 36, 12),
        S::Warning,
        K::VoterUnreachable,
        Some("data-13"),
        5,
        3,
    );
    e.term = Some(64);
    e.leader = Some(node_name(3));
    e.voters = voters(&[3, 4, 5]);
    out.push(e);

    let mut e = event(
        98144,
        at(13, 36, 11),
        S::Warning,
        K::VoterUnreachable,
        Some("data-12"),
        5,
        2,
    );
    e.term = Some(91);
    e.leader = Some(node_name(2));
    e.voters = voters(&[2, 4, 5]);
    out.push(e);

    out.push(event(
        98143,
        at(13, 36, 2),
        S::Error,
        K::NodeUnreachable,
        None,
        5,
        1,
    ));

    let mut e = event(
        98142,
        at(13, 35, 41),
        S::Info,
        K::LeaderElected,
        Some("data-12"),
        2,
        2,
    );
    e.term = Some(91);
    e.previous_leader = Some(node_name(5));
    e.leader = Some(node_name(2));
    e.voters = voters(&[2, 4, 5]);
    e.correlation = Some("op_72".to_string());
    out.push(e);

    out.push(event(
        98141,
        at(13, 35, 9),
        S::Info,
        K::NodeConnected,
        None,
        4,
        1,
    ));

    let mut e = event(
        98140,
        at(13, 21, 7),
        S::Info,
        K::PlacementJobCompleted,
        Some("data-11"),
        1,
        1,
    );
    e.term = Some(87);
    e.leader = Some(node_name(1));
    e.voters = voters(&[1, 2, 4]);
    e.correlation = Some("op_70".to_string());
    e.actor = Some("admin".to_string());
    out.push(e);

    let mut e = event(
        98138,
        at(12, 44, 30),
        S::Info,
        K::ConfigChangeApplied,
        Some("metadata-0"),
        1,
        1,
    );
    e.term = Some(142);
    e.leader = Some(node_name(1));
    e.voters = voters(&[1, 2, 3]);
    e.correlation = Some("op_69".to_string());
    e.actor = Some("admin".to_string());
    out.push(e);

    let mut e = event(
        98133,
        at(12, 10, 2),
        S::Info,
        K::SnapshotCreated,
        Some("data-13"),
        3,
        3,
    );
    e.term = Some(64);
    e.leader = Some(node_name(3));
    e.voters = voters(&[3, 4, 5]);
    e.correlation = Some("op_68".to_string());
    e.actor = Some("admin".to_string());
    out.push(e);

    let mut e = event(
        98127,
        at(11, 52, 18),
        S::Info,
        K::LeadershipTransferred,
        Some("data-13"),
        3,
        3,
    );
    e.term = Some(64);
    e.previous_leader = Some(node_name(4));
    e.leader = Some(node_name(3));
    e.voters = voters(&[3, 4, 5]);
    e.correlation = Some("op_67".to_string());
    e.actor = Some("admin".to_string());
    out.push(e);

    out.push(event(
        98101,
        epoch(2026, 6, 12, 9, 41, 3),
        S::Info,
        K::NodeConnected,
        None,
        5,
        1,
    ));
    out
}
