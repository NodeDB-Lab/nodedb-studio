//! Cluster → Nodes: configured members, the Raft groups the selected member
//! hosts, and that member's details.
//!
//! Three panels: Nodes and Replication groups stacked on the left, Node
//! details on the right. The selection lives here, above the panels, so
//! hiding a panel through the View menu keeps it.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::models::cluster::{ClusterMember, HostedGroup, NodeState};
use crate::models::format::ago;
use crate::routes::Route;
use crate::routes::tabs::ClusterTab;
use crate::services::backend::Backend;
use crate::state::workspace::Surface;
use crate::views::cluster::node_details::NodeDetails;

const SURFACE: Surface = Surface::Cluster(ClusterTab::Nodes);
const GROUPS_SIZE: PanelSize = PanelSize::height(300.0, 160.0, 560.0);
const DETAILS_SIZE: PanelSize = PanelSize::width(360.0, 280.0, 560.0);

/// Members whose name, endpoint, or zone contains `filter`, ignoring case.
fn matching<'a>(members: &'a [ClusterMember], filter: &str) -> Vec<&'a ClusterMember> {
    let needle = filter.trim().to_lowercase();
    members
        .iter()
        .filter(|m| {
            needle.is_empty()
                || [&m.name, &m.endpoint, &m.zone]
                    .iter()
                    .any(|f| f.to_lowercase().contains(&needle))
        })
        .collect()
}

/// The group to show in details: the chosen one while the member still
/// hosts it, else the first group this member leads, else the first group.
pub fn effective_group<'a>(
    groups: &'a [HostedGroup],
    chosen: Option<&str>,
) -> Option<&'a HostedGroup> {
    chosen
        .and_then(|id| groups.iter().find(|g| g.group == id))
        .or_else(|| {
            groups
                .iter()
                .find(|g| g.role_here == crate::models::cluster::GroupRole::Leader)
        })
        .or_else(|| groups.first())
}

fn state_marker(state: NodeState) -> Element {
    match state {
        NodeState::Online => rsx! { span { class: "status-dot ok" } },
        NodeState::CatchingUp => {
            rsx! { span { class: "state-icon warn", Ico { icon: Icon::CircleAlert, size: 16 } } }
        }
        NodeState::Unreachable => {
            rsx! { span { class: "state-icon muted", Ico { icon: Icon::Ban, size: 16 } } }
        }
    }
}

#[component]
pub fn NodesTab() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let nav = use_navigator();
    let mut filter = use_signal(String::new);
    let mut chosen_node = use_signal(|| None::<String>);
    let mut chosen_group = use_signal(|| None::<String>);

    let mut members = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            // Membership belongs to the connection: a switch re-reads it.
            let _conn = conn();
            async move { backend.cluster_members().await }
        })
    };
    let state = resource_state(&members);
    let list = state.loaded().cloned();

    // The chosen member while it is still listed, else the first one. A
    // memo, so the groups read re-runs only when the selection changes.
    // Reading the resource and the chosen id inside subscribes it to both.
    let selected_id = use_memo(move || {
        let chosen = chosen_node.read().clone();
        let read = members.read();
        let list = read.as_ref().and_then(|r| r.as_ref().ok())?;
        chosen
            .filter(|id| list.members.iter().any(|m| &m.id == id))
            .or_else(|| list.members.first().map(|m| m.id.clone()))
    });
    let selected: Option<ClusterMember> = list.as_ref().and_then(|l| {
        let id = selected_id.read().clone()?;
        l.members.iter().find(|m| m.id == id).cloned()
    });

    let mut groups = use_resource(move || {
        let backend = backend.clone();
        let id = selected_id();
        async move {
            match id {
                Some(id) => backend.hosted_groups(&id).await,
                None => Ok(Vec::new()),
            }
        }
    });
    let group_state = resource_state(&groups);
    let group_list: Vec<HostedGroup> = group_state.loaded().cloned().unwrap_or_default();
    let group = effective_group(&group_list, chosen_group.read().as_deref()).cloned();

    let observed = list
        .as_ref()
        .map(|l| l.observed_at.clone())
        .unwrap_or_default();
    let counts = list
        .as_ref()
        .map(|l| (l.members.len(), l.reachable()))
        .unwrap_or_default();
    let selected_name = selected
        .as_ref()
        .map(|m| m.name.clone())
        .unwrap_or_default();

    rsx! {
        div { class: "page-body",
            div { class: "workbench",
                div { class: "workbench col",
                    Panel {
                        surface: SURFACE,
                        id: "nodes",
                        title: "Nodes",
                        class: "fill",
                        actions: rsx! {
                            if !observed.is_empty() {
                                span { "Observed {observed}" }
                            }
                        },
                        div { class: "toolbar nowrap",
                            button { class: "select", title: "Cluster operations span every database",
                                span { class: "select-label", "Cluster scope ·" }
                                span { "All databases" }
                                Ico { icon: Icon::ChevronDown, size: 16 }
                            }
                            div { class: "search-field grow",
                                Ico { icon: Icon::Search, size: 16 }
                                input {
                                    class: "input",
                                    placeholder: "Filter nodes…",
                                    value: "{filter}",
                                    oninput: move |e| filter.set(e.value()),
                                }
                            }
                            div { class: "toolbar-spacer" }
                            button { class: "btn", onclick: move |_| members.restart(),
                                Ico { icon: Icon::RefreshCw, size: 16 }
                                "Refresh"
                            }
                            button {
                                class: "btn",
                                disabled: true,
                                title: "Membership changes need a permission check and an impact preview, which are not designed yet",
                                "Manage membership"
                                Ico { icon: Icon::ChevronDown, size: 16 }
                            }
                        }
                        AsyncView {
                            loading: state.is_loading(),
                            empty: state.is_empty(),
                            error: state.error_message(),
                            retriable: state.is_retriable(),
                            on_retry: move |_| members.restart(),
                            empty_message: "This cluster reports no configured members.",
                        }
                        if let Some(l) = list.as_ref() {
                            div { class: "dt-wrap",
                                table { class: "dt",
                                    thead {
                                        tr {
                                            th { "Node" }
                                            th { "Endpoint" }
                                            th { "Zone" }
                                            th { "State" }
                                            th { "Last seen" }
                                        }
                                    }
                                    tbody {
                                        for m in matching(&l.members, &filter.read()) {
                                            {
                                                let id = m.id.clone();
                                                let is_selected = Some(&m.id) == selected.as_ref().map(|s| &s.id);
                                                rsx! {
                                                    tr {
                                                        key: "{m.id}",
                                                        class: if is_selected { "clickable selected" } else { "clickable" },
                                                        "aria-selected": "{is_selected}",
                                                        onclick: move |_| {
                                                            chosen_node.set(Some(id.clone()));
                                                            chosen_group.set(None);
                                                        },
                                                        td { class: "key", "{m.name}" }
                                                        td { class: "mono", "{m.endpoint}" }
                                                        td { "{m.zone}" }
                                                        td { span { class: "status", {state_marker(m.state)} "{m.state.label()}" } }
                                                        td { "{ago(m.last_seen_secs)}" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            div { class: "dt-footer",
                                "{counts.0} configured nodes · {counts.1} reachable"
                            }
                        }
                    }
                    ResizeHandle { surface: SURFACE, target: "groups", size: GROUPS_SIZE, side: Side::After }
                    Panel {
                        surface: SURFACE,
                        id: "groups",
                        title: "Replication groups",
                        size: GROUPS_SIZE,
                        div { class: "panel-subtitle", "Groups hosted on {selected_name}" }
                        AsyncView {
                            loading: group_state.is_loading(),
                            empty: group_state.is_empty() && selected.is_some(),
                            error: group_state.error_message(),
                            retriable: group_state.is_retriable(),
                            on_retry: move |_| groups.restart(),
                            empty_message: "This node hosts no Raft groups.",
                        }
                        if !group_list.is_empty() {
                            div { class: "dt-wrap",
                                table { class: "dt",
                                    thead {
                                        tr {
                                            th { "Group" }
                                            th { "Kind" }
                                            th { "Role here" }
                                            th { "Leader" }
                                            th { "Replication state" }
                                        }
                                    }
                                    tbody {
                                        for g in group_list.iter() {
                                            {
                                                let id = g.group.clone();
                                                let is_selected = group.as_ref().is_some_and(|s| s.group == g.group);
                                                let dot = if g.state.healthy() { "status-dot ok" } else { "status-dot warn" };
                                                rsx! {
                                                    tr {
                                                        key: "{g.group}",
                                                        class: if is_selected { "clickable selected" } else { "clickable" },
                                                        "aria-selected": "{is_selected}",
                                                        onclick: move |_| chosen_group.set(Some(id.clone())),
                                                        td { class: "key", "{g.group}" }
                                                        td { "{g.kind.label()}" }
                                                        td { "{g.role_here.label()}" }
                                                        td { "{g.leader}" }
                                                        td { span { class: "status", span { class: "{dot}" } "{g.state.label()}" } }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            div { class: "panel-footer",
                                button {
                                    class: "link-btn",
                                    onclick: move |_| { nav.push(Route::Cluster { tab: ClusterTab::RaftGroups }); },
                                    Ico { icon: Icon::ExternalLink, size: 16 }
                                    "Open selected Raft group"
                                }
                            }
                        }
                    }
                }
                ResizeHandle { surface: SURFACE, target: "details", size: DETAILS_SIZE, side: Side::After }
                Panel {
                    surface: SURFACE,
                    id: "details",
                    title: "Node details",
                    size: DETAILS_SIZE,
                    if let Some(m) = selected {
                        NodeDetails { member: m, group }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::MockConnectionService;
    use crate::services::cluster_data::ClusterData;

    #[tokio::test]
    async fn filter_matches_name_endpoint_and_zone() {
        let svc = MockConnectionService::ready();
        let list = svc.cluster_members().await.expect("members");
        assert_eq!(matching(&list.members, "").len(), list.members.len());
        assert_eq!(matching(&list.members, "NODE-2").len(), 1);
        assert_eq!(matching(&list.members, "10.4.0.13").len(), 1);
        assert_eq!(matching(&list.members, "sg-a").len(), 2);
        assert!(matching(&list.members, "nope").is_empty());
    }

    #[tokio::test]
    async fn group_falls_back_to_the_led_group() {
        let svc = MockConnectionService::ready();
        let groups = svc.hosted_groups("nd_02").await.expect("groups");
        let pick = |chosen| effective_group(&groups, chosen).map(|g| g.group.clone());
        assert_eq!(pick(None).as_deref(), Some("data-12"));
        assert_eq!(pick(Some("sequencer-0")).as_deref(), Some("sequencer-0"));
        // A group this member does not host falls back, never shows stale.
        assert_eq!(pick(Some("data-99")).as_deref(), Some("data-12"));
    }
}
