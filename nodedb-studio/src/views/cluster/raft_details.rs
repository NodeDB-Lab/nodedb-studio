//! The Raft group details: facts, peers, configuration, and snapshots of the
//! one selected group, and the actions that apply to that group alone.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::components::tabs::TabBar;
use crate::models::cluster_place::size_text;
use crate::models::cluster_raft::{PeerRole, RaftGroup};
use crate::models::format::{date_second, thousands};
use crate::services::backend::Backend;
use crate::state::cluster::{ClusterUi, RaftTab};
use crate::views::cluster::common::{Fact, NameChips};
use crate::views::cluster::raft_tasks;
use crate::views::events::common::{StatusBadge, StatusText};

#[component]
pub fn RaftDetails(group: RaftGroup) -> Element {
    let mut ui = use_context::<ClusterUi>().raft;
    let tab = ui.read().tab;
    let health = group.health();
    rsx! {
        div { class: "cl-details",
            div { class: "inspector-head",
                h3 { class: "inspector-title mono", "{group.id}" }
                span { class: "badge gray", "{group.kind.label()}" }
                div { class: "toolbar-spacer" }
                StatusBadge { tone: health.tone(), text: health.label().to_string() }
            }
            TabBar {
                items: RaftTab::ALL.iter().map(|t| t.label()).collect(),
                active: tab.index(),
                on_select: move |i: usize| ui.write().tab = RaftTab::ALL[i],
            }
            match tab {
                RaftTab::Overview => rsx! { Overview { group: group.clone() } },
                RaftTab::Configuration => rsx! { Configuration { group: group.clone() } },
                RaftTab::Snapshots => rsx! { Snapshots { group: group.clone() } },
            }
        }
    }
}

#[component]
fn Overview(group: RaftGroup) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let mut ui = use_context::<ClusterUi>().raft;
    let g = &group;
    let health = g.health();
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
                Fact { label: "Replicas", span { class: "num", "{g.peers.len()}" } }
                Fact { label: "Leader", "{leader}" }
                Fact { label: "State", StatusText { tone: health.tone(), text: health.label().to_string() } }
            }
            div { class: "cl-fact-col",
                Fact { label: "Term", span { class: "num", "{g.term}" } }
                Fact { label: "Voters", span { class: "num", "{g.voter_count()}" } }
                Fact { label: "Quorum", span { class: "num", "{g.quorum()} of {g.voter_count()}" } }
                Fact { label: "Reachable", span { class: "num", "{g.reachable_voters()} of {g.voter_count()}" } }
            }
            div { class: "cl-fact-col",
                Fact { label: "Commit index", span { class: "num", "{thousands(g.commit_index)}" } }
                Fact { label: "Applied index", span { class: "num", "{thousands(g.applied_index)}" } }
                Fact { label: "Pending entries", span { class: "num", "{thousands(g.pending_entries())}" } }
            }
        }
        h4 { class: "inspector-section", "Peers" }
        div { class: "dt-wrap cl-peers",
            table { class: "dt",
                thead {
                    tr {
                        th { "Node" }
                        th { "Role" }
                        th { class: "num", "Match index" }
                        th { class: "num", "Applied index" }
                        th { class: "num", title: "Index lag: entries the match index trails the commit index by", "Lag (entries)" }
                        th { class: "num", title: "Heartbeat age: milliseconds since the leader last heard from the peer", "Heartbeat (ms)" }
                        th { "State" }
                    }
                }
                tbody {
                    for p in g.peers.iter() {
                        {
                            let state = p.state();
                            rsx! {
                                tr { key: "{p.node_id}", class: if p.role == PeerRole::Leader { "selected" } else { "" },
                                    td { class: "key", "{p.node}" }
                                    td { "{p.role.label()}" }
                                    td { class: "num", "{thousands(p.match_index)}" }
                                    td { class: "num", "{thousands(p.applied_index)}" }
                                    td { class: "num", "{thousands(g.match_lag(p))}" }
                                    td { class: "num",
                                        match (p.role, p.heartbeat_age_ms) {
                                            (PeerRole::Leader, _) => rsx! { span { class: "muted", title: "The leader does not heartbeat itself", "—" } },
                                            (_, Some(ms)) => rsx! { "{thousands(ms)}" },
                                            (_, None) => rsx! { span { class: "muted", title: "No heartbeat answer. Index values are the last the leader recorded.", "Unknown" } },
                                        }
                                    }
                                    td { StatusText { tone: state.tone(), text: state.label().to_string() } }
                                }
                            }
                        }
                    }
                }
            }
        }
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

#[component]
fn Configuration(group: RaftGroup) -> Element {
    let g = &group;
    let voters: Vec<String> = g.voters().iter().map(|p| p.node.clone()).collect();
    let learners: Vec<String> = g.learners().iter().map(|p| p.node.clone()).collect();
    let leader = g.leader_name().to_string();
    rsx! {
        div { class: "kv cl-config",
            div { class: "kv-row", span { "Group" } span { class: "mono", "{g.id}" } }
            div { class: "kv-row", span { "Group type" } span { "{g.kind.label()}" } }
            div { class: "kv-row", span { "Configuration index" } span { class: "num", "{thousands(g.config_index)}" } }
            div { class: "kv-row", span { "Voters" } NameChips { names: voters, highlight: Some(leader) } }
            div { class: "kv-row",
                span { "Learners" }
                if learners.is_empty() {
                    span { class: "muted", "None" }
                } else {
                    NameChips { names: learners }
                }
            }
            div { class: "kv-row", span { "Quorum" } span { class: "num", "{g.quorum()} of {g.voter_count()} voters" } }
        }
        p { class: "inspector-note",
            "A learner replicates the log and never votes. The highlighted voter is the leader. Membership changes start from Placement, as a reviewed replica move."
        }
    }
}

#[component]
fn Snapshots(group: RaftGroup) -> Element {
    let g = &group;
    rsx! {
        if g.snapshots.is_empty() {
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
                        for s in g.snapshots.iter() {
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
