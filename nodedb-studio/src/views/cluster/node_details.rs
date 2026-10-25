//! The Node details inspector: the selected member and its selected group.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::cluster::{ClusterMember, HostedGroup, NodeState};
use crate::models::format::thousands;
use crate::routes::Route;
use crate::routes::tabs::ClusterTab;
use crate::state::cluster::ClusterUi;

fn state_badge(state: NodeState) -> (&'static str, &'static str) {
    match state {
        NodeState::Online => ("badge green", "status-dot ok"),
        NodeState::CatchingUp => ("badge amber", "status-dot warn"),
        NodeState::Unreachable => ("badge gray", "status-dot idle"),
    }
}

#[component]
pub fn NodeDetails(member: ClusterMember, group: Option<HostedGroup>) -> Element {
    let nav = use_navigator();
    let mut raft = use_context::<ClusterUi>().raft;
    let (badge, dot) = state_badge(member.state);
    rsx! {
        div { class: "inspector",
            div { class: "inspector-head",
                h3 { class: "inspector-title", "{member.name}" }
                span { class: "{badge}", span { class: "{dot}" } "{member.state.label()}" }
            }
            div { class: "kv",
                div { class: "kv-row", span { "Node ID" } span { class: "mono", "{member.id}" } }
                div { class: "kv-row", span { "Endpoint" } span { class: "mono", "{member.endpoint}" } }
                div { class: "kv-row", span { "Zone" } span { "{member.zone}" } }
                div { class: "kv-row", span { "Kind" } span { "{member.kind.label()}" } }
                div { class: "kv-row", span { "Last heartbeat" } span { "{member.last_heartbeat}" } }
                div { class: "kv-row", span { "Hosted groups" } span { class: "num", "{member.hosted_groups}" } }
            }
            if let Some(g) = group.as_ref() {
                h4 { class: "inspector-section", "Selected group" }
                div { class: "kv",
                    div { class: "kv-row", span { "Group" } span { "{g.group}" } }
                    div { class: "kv-row", span { "Role" } span { "{g.role_here.label()}" } }
                    div { class: "kv-row", span { "Term" } span { class: "num", "{g.term}" } }
                    div { class: "kv-row", span { "Commit index" } span { class: "num", "{thousands(g.commit_index)}" } }
                    div { class: "kv-row", span { "Applied index" } span { class: "num", "{thousands(g.applied_index)}" } }
                }
            }
            p { class: "inspector-note", "Observed on {member.name}." }
            div { class: "inspector-actions",
                button {
                    class: "btn",
                    disabled: true,
                    title: "Server log streaming is not available yet",
                    Ico { icon: Icon::FileText, size: 16 }
                    "View logs"
                }
                button {
                    class: "btn primary",
                    disabled: group.is_none(),
                    onclick: {
                        let id = group.as_ref().map(|g| g.group.clone());
                        move |_| {
                            raft.write().group = id.clone();
                            nav.push(Route::Cluster { tab: ClusterTab::RaftGroups });
                        }
                    },
                    Ico { icon: Icon::ExternalLink, size: 16 }
                    "Open group"
                }
            }
        }
    }
}
