//! The Raft group details: facts, peers, configuration, and snapshots of the
//! one selected group, as the observer holds them, and the actions that
//! apply to that group alone.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::components::tabs::TabBar;
use crate::models::cluster::Observer;
use crate::models::cluster::place::size_text;
use crate::models::cluster::raft::{PeerRole, RaftGroup};
use crate::models::format::{date_second, thousands};
use crate::services::backend::Backend;
use crate::state::caps::cluster::offers_cluster_ops;
use crate::state::cluster::{ClusterUi, RaftTab};
use crate::state::gates::use_caps;
use crate::views::cluster::common::{Fact, NameChips, NotReported, SeenBy, or_not_reported};
use crate::views::cluster::raft_tasks;
use crate::views::events::common::{StatusBadge, StatusText};

/// Why a peer's log position is missing.
const NO_PROGRESS: &str = "NodeDB 0.6.2 has no statement for the leader's record of each peer";

#[component]
pub fn RaftDetails(group: RaftGroup, observer: Observer) -> Element {
    let mut ui = use_context::<ClusterUi>().raft;
    let tab = ui.read().tab;
    rsx! {
        div { class: "cl-details",
            div { class: "inspector-head",
                h3 { class: "inspector-title mono", "{group.id}" }
                span { class: "badge gray", "{group.kind.label()}" }
                div { class: "toolbar-spacer" }
                SeenBy { observer: observer.clone() }
                if let Some(h) = group.health() {
                    StatusBadge { tone: h.tone(), text: h.label().to_string() }
                }
            }
            TabBar {
                items: RaftTab::ALL.iter().map(|t| t.label()).collect(),
                active: tab.index(),
                on_select: move |i: usize| ui.write().tab = RaftTab::ALL[i],
            }
            match tab {
                RaftTab::Overview => rsx! { Overview { group: group.clone(), observer: observer.clone() } },
                RaftTab::Configuration => rsx! { Configuration { group: group.clone() } },
                RaftTab::Snapshots => rsx! { Snapshots { group: group.clone() } },
            }
        }
    }
}

#[component]
fn Overview(group: RaftGroup, observer: Observer) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let mut ui = use_context::<ClusterUi>().raft;
    let ops = offers_cluster_ops(use_caps().as_ref());
    let g = &group;
    let on = &observer.node_id;
    let targets: Vec<(String, String)> = g
        .transfer_targets()
        .iter()
        .map(|p| (p.node_id.clone(), p.node.clone()))
        .collect();
    let chosen = ui
        .read()
        .target
        .clone()
        .filter(|t| targets.iter().any(|(id, _)| id == t))
        .or_else(|| targets.first().map(|(id, _)| id.clone()));
    let snapshotting = ui.read().snapshotting;
    let leader = g.leader_name().to_string();
    rsx! {
        div { class: "cl-facts",
            div { class: "cl-fact-col",
                Fact { label: "Group", span { class: "mono", "{g.id}" } }
                Fact { label: "Replicas",
                    span { class: "num", {or_not_reported(g.peers.as_ref().filter(|_| g.learners_reported), |p| p.len().to_string())} }
                }
                Fact { label: "Leader", "{leader}" }
                Fact { label: "State",
                    match g.health() {
                        Some(h) => rsx! { StatusText { tone: h.tone(), text: h.label().to_string() } },
                        None => rsx! { NotReported {} },
                    }
                }
            }
            div { class: "cl-fact-col",
                Fact { label: "Term", span { class: "num", "{g.term}" } }
                Fact { label: "Voters", span { class: "num", "{g.voter_count}" } }
                Fact { label: "Quorum", span { class: "num", "{g.quorum()} of {g.voter_count}" } }
                Fact { label: "Reachable",
                    span { class: "num", {or_not_reported(g.reachable_voters(), |up| format!("{up} of {}", g.voter_count))} }
                }
            }
            div { class: "cl-fact-col",
                Fact { label: "Commit index", span { class: "num", "{thousands(g.commit_index)}" } }
                Fact { label: "Applied index", span { class: "num", "{thousands(g.applied_index)}" } }
                Fact { label: "Pending entries", span { class: "num", "{thousands(g.pending_entries())}" } }
            }
        }
        h4 { class: "inspector-section", "Peers" }
        if g.peers.is_none() {
            div { class: "async-empty", "Node {on} does not list the members of {g.id}." }
        }
        div { class: "dt-wrap cl-peers",
            table { class: "dt",
                thead {
                    tr {
                        th { "Node" }
                        th { title: "The leader is the one node {on} knows", "Role" }
                        th { class: "num", "Match index" }
                        th { class: "num", "Applied index" }
                        th { class: "num", title: "Index lag: entries the match index trails the commit index by", "Lag (entries)" }
                        th { class: "num", title: "Heartbeat age: milliseconds since the leader last heard from the peer", "Heartbeat (ms)" }
                        th { title: "Node {on}'s circuit breaker to the peer", "Circuit from node {on}" }
                    }
                }
                tbody {
                    for p in g.peers().iter() {
                        tr { key: "{p.node_id}", class: if p.role == PeerRole::Leader { "selected" } else { "" },
                            td { class: "key", "{p.node}" }
                            td { "{p.role.label()}" }
                            match p.progress {
                                Some(x) => rsx! {
                                    td { class: "num", "{thousands(x.match_index)}" }
                                    td { class: "num", "{thousands(x.applied_index)}" }
                                    td { class: "num", {or_not_reported(g.match_lag(p), thousands)} }
                                    td { class: "num",
                                        match (p.role, x.heartbeat_age_ms) {
                                            (PeerRole::Leader, _) => rsx! { span { class: "muted", title: "The leader does not heartbeat itself", "—" } },
                                            (_, Some(ms)) => rsx! { "{thousands(ms)}" },
                                            (_, None) => rsx! { span { class: "muted", title: "No heartbeat answer. Index values are the last the leader recorded.", "Unknown" } },
                                        }
                                    }
                                },
                                None => rsx! {
                                    for col in ["match", "applied", "lag", "heartbeat"] {
                                        td { key: "{col}", class: "num", NotReported { why: NO_PROGRESS } }
                                    }
                                },
                            }
                            td { StatusText { tone: p.link.tone(), text: p.link.label() } }
                        }
                    }
                }
            }
        }
        p { class: "inspector-note",
            "Term, commit index and applied index are node {on}'s. The leader and the circuits are node {on}'s view: another node can hold a different one during a change."
        }
        if ops {
        div { class: "cl-actions",
            div { class: "cl-action",
                div { class: "cl-action-head",
                    h4 { "Leadership" }
                    button {
                        class: "btn primary",
                        disabled: chosen.is_none(),
                        title: if chosen.is_none() { "No other voter of this group is reachable" } else { "Review the transfer before it applies" },
                        onclick: {
                            let (backend, group_id, chosen) = (backend.clone(), g.id.clone(), chosen.clone());
                            move |_| {
                                let Some(target) = chosen.clone() else { return };
                                let (backend, group_id) = (backend.clone(), group_id.clone());
                                spawn_forever(async move {
                                    raft_tasks::open_transfer(ui, backend, group_id, target).await
                                });
                            }
                        },
                        Ico { icon: Icon::Move, size: 16 }
                        "Transfer leadership"
                    }
                }
                p { class: "hint", "Transfer leadership of {g.id} to another voter. Other groups keep their leaders." }
                div { class: "cl-ctl",
                    label { r#for: "raft-target", "Target node" }
                    select {
                        id: "raft-target",
                        class: "select native",
                        disabled: targets.is_empty(),
                        onchange: move |e| ui.write().target = Some(e.value()),
                        for (id, name) in targets.iter() {
                            option { key: "{id}", value: "{id}", selected: Some(id) == chosen.as_ref(), "{name}" }
                        }
                        if targets.is_empty() {
                            option { value: "", "No eligible voter" }
                        }
                    }
                }
            }
            div { class: "cl-action",
                div { class: "cl-action-head",
                    h4 { "Snapshot" }
                    button {
                        class: "btn",
                        disabled: snapshotting,
                        onclick: {
                            let (backend, group_id) = (backend.clone(), g.id.clone());
                            move |_| {
                                let (backend, group_id) = (backend.clone(), group_id.clone());
                                spawn_forever(async move { raft_tasks::snapshot(ui, backend, group_id).await });
                            }
                        },
                        Ico { icon: Icon::Database, size: 16 }
                        "Create snapshot"
                    }
                }
                p { class: "hint", "Snapshot of group {g.id} at the leader's applied index {thousands(g.applied_index)}." }
                div { class: "cl-ctl",
                    label { "Snapshot type" }
                    span { class: "cl-static", "Full snapshot" }
                }
            }
        }
        }
    }
}

#[component]
fn Configuration(group: RaftGroup) -> Element {
    let g = &group;
    let reported = g.peers.is_some();
    let voters: Vec<String> = g.voters().iter().map(|p| p.node.clone()).collect();
    let learners: Vec<String> = g.learners().iter().map(|p| p.node.clone()).collect();
    let leader = g.leader_name().to_string();
    rsx! {
        div { class: "kv cl-config",
            div { class: "kv-row", span { "Group" } span { class: "mono", "{g.id}" } }
            div { class: "kv-row", span { "Group type" } span { "{g.kind.label()}" } }
            div { class: "kv-row", span { "Configuration index" } span { class: "num", {or_not_reported(g.config_index, thousands)} } }
            div { class: "kv-row",
                span { "Voters" }
                if reported {
                    NameChips { names: voters, highlight: Some(leader) }
                } else {
                    NotReported {}
                }
            }
            div { class: "kv-row",
                span { "Learners" }
                if !reported || !g.learners_reported {
                    NotReported {}
                } else if learners.is_empty() {
                    span { class: "muted", "None" }
                } else {
                    NameChips { names: learners }
                }
            }
            div { class: "kv-row", span { "Quorum" } span { class: "num", "{g.quorum()} of {g.voter_count} voters" } }
        }
        p { class: "inspector-note",
            "A learner replicates the log and never votes. The highlighted voter is the leader. Membership changes start from Placement, as a reviewed replica move."
        }
    }
}

#[component]
fn Snapshots(group: RaftGroup) -> Element {
    let g = &group;
    let snapshots = g.snapshots.clone().unwrap_or_default();
    rsx! {
        if g.snapshots.is_none() {
            div { class: "async-empty", "Snapshots are not reported. NodeDB 0.6.2 has no statement that lists them." }
        } else if snapshots.is_empty() {
            div { class: "async-empty", "No snapshot of {g.id} exists." }
        } else {
            div { class: "dt-wrap",
                table { class: "dt",
                    thead {
                        tr {
                            th { "Snapshot" }
                            th { class: "num", "Last index" }
                            th { "Taken (UTC)" }
                            th { class: "num", "Size" }
                            th { "Node" }
                        }
                    }
                    tbody {
                        for s in snapshots.iter() {
                            tr { key: "{s.id}",
                                td { class: "key mono", "{s.id}" }
                                td { class: "num", "{thousands(s.index)}" }
                                td { "{date_second(s.taken_at, 0)}" }
                                td { class: "num", "{size_text(s.size_bytes)}" }
                                td { "{s.node}" }
                            }
                        }
                    }
                }
            }
        }
        p { class: "inspector-note",
            "A snapshot covers {g.id} only. A peer that falls behind the compacted log installs one to catch up."
        }
    }
}
