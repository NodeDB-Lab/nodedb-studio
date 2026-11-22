//! The Node details inspector: the selected member and its selected group,
//! as the observer sees the member.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::cluster::{ClusterMember, HostedGroup, Observer};
use crate::models::format::thousands;
use crate::routes::Route;
use crate::routes::tabs::ClusterTab;
use crate::state::cluster::ClusterUi;
use crate::views::cluster::common::{SeenBy, or_not_reported};
use crate::views::events::common::{StatusBadge, StatusText};

/// What an open circuit and `draining` do and do not tell. A stopped peer
/// shows exactly these two, so neither can mean the node is fine.
pub const PEER_STATES: &str = "A peer that stops answering opens the circuit first and turns draining later. Neither state tells you whether the node is down or was removed.";

#[component]
pub fn NodeDetails(
    member: ClusterMember,
    observer: Observer,
    group: Option<HostedGroup>,
) -> Element {
    let nav = use_navigator();
    let mut raft = use_context::<ClusterUi>().raft;
    let on = &observer.node_id;
    rsx! {
        div { class: "inspector",
            div { class: "inspector-head",
                h3 { class: "inspector-title", "{member.name}" }
                StatusBadge { tone: member.state.tone(), text: member.state.label().to_string() }
            }
            div { class: "kv",
                div { class: "kv-row", span { "Node ID" } span { class: "mono", "{member.id}" } }
                div { class: "kv-row", span { "Peer address" } span { class: "mono", "{member.address}" } }
                div { class: "kv-row",
                    span { title: "The state node {on} holds for this member", "Node state" }
                    StatusText { tone: member.state.tone(), text: member.state.label().to_string() }
                }
                div { class: "kv-row",
                    span { title: "Node {on}'s circuit breaker to this member", "Circuit from node {on}" }
                    StatusText { tone: member.link.tone(), text: member.link.label() }
                }
                div { class: "kv-row", span { "Zone" } span { {or_not_reported(member.zone.clone(), |z| z)} } }
                div { class: "kv-row", span { "Kind" } span { "{member.kind.label()}" } }
                div { class: "kv-row", span { "Last heartbeat" } span { {or_not_reported(member.last_heartbeat.clone(), |h| h)} } }
                div { class: "kv-row",
                    span { "Hosted groups" }
                    span { class: "num", {or_not_reported(member.hosted_groups, |n| n.to_string())} }
                }
            }
            if let Some(g) = group.as_ref() {
                h4 { class: "inspector-section", "Selected group, as {member.name} sees it" }
                div { class: "kv",
                    div { class: "kv-row", span { "Group" } span { "{g.group}" } }
                    div { class: "kv-row", span { "Role" } span { "{g.role_here.label()}" } }
                    div { class: "kv-row", span { "Term" } span { class: "num", "{g.term}" } }
                    div { class: "kv-row", span { "Commit index" } span { class: "num", "{thousands(g.commit_index)}" } }
                    div { class: "kv-row", span { "Applied index" } span { class: "num", "{thousands(g.applied_index)}" } }
                }
            }
            p { class: "inspector-note",
                SeenBy { observer: observer.clone() }
                " {PEER_STATES}"
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A killed peer shows exactly an open circuit, then `draining`: the
    /// note must not say the node is not gone.
    #[test]
    fn the_peer_note_does_not_say_the_node_is_up() {
        assert!(!PEER_STATES.contains("Neither means the node is gone"));
        assert!(PEER_STATES.contains("whether the node is down or was removed"));
    }
}
