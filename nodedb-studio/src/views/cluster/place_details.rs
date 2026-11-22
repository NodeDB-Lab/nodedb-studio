//! The vShard details and the Nodes health table of the Placement tab.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::cluster::Link;
use crate::models::cluster::place::{Placement, VShard, size_text};
use crate::models::cluster::raft::PeerRole;
use crate::routes::Route;
use crate::routes::tabs::ClusterTab;
use crate::state::cluster::ClusterUi;
use crate::views::cluster::common::{NotReported, or_not_reported, routing_leader};
use crate::views::events::common::{StatusBadge, StatusText};

#[component]
pub fn VShardDetails(placement: Placement, vshard: VShard) -> Element {
    let nav = use_navigator();
    let mut raft = use_context::<ClusterUi>().raft;
    let v = &vshard;
    let status = v.status();
    let group = v.group.clone();
    let members: Vec<(String, Option<PeerRole>, Link)> = placement
        .nodes
        .iter()
        .filter_map(|n| {
            n.role_in(&v.group)
                .map(|r| (n.name.clone(), Some(r), n.link.clone()))
        })
        .collect();
    let quorum = placement.quorum(&v.group).cloned();
    let on = placement.observer.node_id.clone();
    let used = match (v.size_bytes, v.used_percent()) {
        (Some(b), Some(pct)) => Some(format!("{} ({pct}%)", size_text(b))),
        (Some(b), None) => Some(size_text(b)),
        (None, _) => None,
    };
    rsx! {
        div { class: "inspector cl-details",
            div { class: "inspector-head",
                h3 { class: "inspector-title", "vShard {v.number}" }
                StatusBadge { tone: status.tone(), text: status.label().to_string() }
            }
            div { class: "kv",
                div { class: "kv-row", span { "Database" } span { {or_not_reported(v.database.clone(), |d| d)} } }
                div { class: "kv-row", span { "Collection" } span { {or_not_reported(v.collection.clone(), |c| c)} } }
                div { class: "kv-row", span { "vShard" } span { class: "num", "{v.number}" } }
                div { class: "kv-row", span { "Group" } span { class: "mono", "{v.group}" } }
                div { class: "kv-row", span { title: routing_leader(&on), "Routing leader" } span { "{v.leader.as_deref().unwrap_or(\"none\")}" } }
                div { class: "kv-row", span { "Replicas" } span { "{v.replicas.join(\" / \")}" } }
                div { class: "kv-row", span { "Storage class" } span { class: "mono", {or_not_reported(v.storage_class.clone(), |c| c)} } }
                div { class: "kv-row", span { "Capacity" } span { class: "num", {or_not_reported(v.capacity_bytes, size_text)} } }
                div { class: "kv-row", span { "Size (used)" } span { class: "num", {or_not_reported(used, |u| u)} } }
                if let Some(m) = v.movement.as_ref() {
                    div { class: "kv-row",
                        span { "Move" }
                        span {
                            {or_not_reported(m.from.clone(), |f| f)}
                            " → "
                            {or_not_reported(m.to.clone(), |t| t)}
                            if let Some(phase) = m.phase.as_ref() {
                                " · {phase}"
                            }
                            if let Some(op) = m.operation.as_ref() {
                                " · "
                                span { class: "mono", "{op}" }
                            }
                        }
                    }
                }
            }
            h4 { class: "inspector-section", "Group membership" }
            div { class: "cl-members",
                for (name , role , link) in members {
                    span {
                        key: "{name}",
                        class: if role == Some(PeerRole::Leader) { "cl-member lead" } else { "cl-member" },
                        title: "Circuit from node {on}: {link.label()}",
                        span { class: link.tone().dot() }
                        "{name}"
                        if let Some(r) = role {
                            span { class: "cl-member-role", "{r.label()}" }
                        }
                    }
                }
            }
            if let Some(q) = quorum {
                p { class: "inspector-note",
                    match q.reachable {
                        Some(up) => rsx! { "Node {on} reaches {up} of the {q.voters} voters of {q.group}. Quorum is {q.quorum}. Node health and group quorum are separate facts." },
                        None => rsx! { "Quorum of {q.group} is {q.quorum} of {q.voters} voters. Node {on} cannot say how many it reaches." },
                    }
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
    let on = placement.observer.node_id.clone();
    rsx! {
        div { class: "dt-wrap",
            table { class: "dt cl-table",
                thead {
                    tr {
                        th { "Node" }
                        th { title: "The state node {on} holds for the member", "Node state" }
                        th { title: "Node {on}'s circuit breaker to the member", "Circuit from node {on}" }
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
                            td { StatusText { tone: n.state.tone(), text: n.state.label().to_string() } }
                            td { StatusText { tone: n.link.tone(), text: n.link.label() } }
                            td { class: "num",
                                match (n.disk_used_bytes, n.disk_percent()) {
                                    (Some(b), Some(pct)) => rsx! { "{size_text(b)} ({pct}%)" },
                                    (Some(b), None) => rsx! { "{size_text(b)}" },
                                    (None, _) => rsx! { NotReported { why: "NodeDB 0.6.2 reports no disk use" } },
                                }
                            }
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
                match q.reachable {
                    Some(up) => rsx! { " · Voters node {on} reaches {up} / {q.voters} · Quorum {q.quorum}" },
                    None => rsx! { " · Voters {q.voters} · Quorum {q.quorum}" },
                }
            }
        }
    }
}
