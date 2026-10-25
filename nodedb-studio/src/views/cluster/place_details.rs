//! The vShard details and the Nodes health table of the Placement tab.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::cluster::NodeState;
use crate::models::cluster_place::{Placement, VShard, size_text};
use crate::models::cluster_raft::PeerRole;
use crate::models::feeds::Tone;
use crate::routes::Route;
use crate::routes::tabs::ClusterTab;
use crate::state::cluster::ClusterUi;
use crate::views::events::common::{StatusBadge, StatusText};

fn node_tone(state: NodeState) -> Tone {
    match state {
        NodeState::Online => Tone::Ok,
        NodeState::CatchingUp => Tone::Warn,
        NodeState::Unreachable => Tone::Idle,
    }
}

#[component]
pub fn VShardDetails(placement: Placement, vshard: VShard) -> Element {
    let nav = use_navigator();
    let mut raft = use_context::<ClusterUi>().raft;
    let v = &vshard;
    let status = v.status();
    let group = v.group.clone();
    let members: Vec<(String, Option<PeerRole>, bool)> = placement
        .nodes
        .iter()
        .filter_map(|n| {
            n.role_in(&v.group)
                .map(|r| (n.name.clone(), Some(r), v.unreachable.contains(&n.name)))
        })
        .collect();
    let quorum = placement.quorum(&v.group).cloned();
    rsx! {
        div { class: "inspector cl-details",
            div { class: "inspector-head",
                h3 { class: "inspector-title", "vShard {v.number}" }
                StatusBadge { tone: status.tone(), text: status.label().to_string() }
            }
            div { class: "kv",
                div { class: "kv-row", span { "Database" } span { "{v.database}" } }
                div { class: "kv-row", span { "Collection" } span { "{v.collection}" } }
                div { class: "kv-row", span { "vShard" } span { class: "num", "{v.number}" } }
                div { class: "kv-row", span { "Group" } span { class: "mono", "{v.group}" } }
                div { class: "kv-row", span { "Leader" } span { "{v.leader}" } }
                div { class: "kv-row", span { "Replicas" } span { "{v.replicas.join(\" / \")}" } }
                div { class: "kv-row", span { "Storage class" } span { class: "mono", "{v.storage_class}" } }
                div { class: "kv-row", span { "Capacity" } span { class: "num", "{size_text(v.capacity_bytes)}" } }
                div { class: "kv-row", span { "Size (used)" } span { class: "num", "{size_text(v.size_bytes)} ({v.used_percent()}%)" } }
                if let Some(m) = v.movement.as_ref() {
                    div { class: "kv-row",
                        span { "Move" }
                        span { "{m.from} → {m.to} · " span { class: "mono", "{m.operation}" } }
                    }
                }
            }
            h4 { class: "inspector-section", "Group membership" }
            div { class: "cl-members",
                for (name , role , down) in members {
                    span {
                        key: "{name}",
                        class: if role == Some(PeerRole::Leader) { "cl-member lead" } else { "cl-member" },
                        title: if down { "This node does not answer" } else { "" },
                        span { class: if down { "status-dot idle" } else { "status-dot ok" } }
                        "{name}"
                        if let Some(r) = role {
                            span { class: "cl-member-role", "{r.label()}" }
                        }
                    }
                }
            }
            if let Some(q) = quorum {
                p { class: "inspector-note",
                    "{q.group} has {q.reachable} of {q.voters} voters reachable. Quorum is {q.quorum}. Node health and group quorum are separate facts."
                }
            }
            div { class: "inspector-actions",
                button {
                    class: "btn",
                    onclick: move |_| {
                        let mut w = raft.write();
                        w.group = Some(group.clone());
                        w.target = None;
                        drop(w);
                        nav.push(Route::Cluster { tab: ClusterTab::RaftGroups });
                    },
                    Ico { icon: Icon::ExternalLink, size: 16 }
                    "Open Raft group"
                }
            }
        }
    }
}

#[component]
pub fn NodesHealth(placement: Placement, group: Option<String>) -> Element {
    let quorum = group.as_ref().and_then(|g| placement.quorum(g)).cloned();
    rsx! {
        div { class: "dt-wrap",
            table { class: "dt cl-table",
                thead {
                    tr {
                        th { "Node" }
                        th { "Health" }
                        th { class: "num", "Disk used" }
                        if let Some(g) = group.as_ref() {
                            th { title: "Role in {g}", "Role" }
                        }
                    }
                }
                tbody {
                    for n in placement.nodes.iter() {
                        tr { key: "{n.node_id}",
                            td { class: "key", "{n.name}" }
                            td { StatusText { tone: node_tone(n.state), text: n.state.label().to_string() } }
                            td { class: "num", "{size_text(n.disk_used_bytes)} ({n.disk_percent()}%)" }
                            if let Some(g) = group.as_ref() {
                                td {
                                    match n.role_in(g) {
                                        Some(r) => rsx! { "{r.label()}" },
                                        None => rsx! { span { class: "muted", "Not a member" } },
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if let Some(q) = quorum {
            div { class: "dt-footer",
                span { class: "mono", "{q.group}" }
                " · Reachable voters {q.reachable} / {q.voters} · Quorum {q.quorum}"
            }
        }
    }
}
