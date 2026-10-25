//! Cluster → Raft groups: every group with its leader and quorum, and the
//! selected group's peers, log positions, and group-scoped actions.
//!
//! Two panels. The selection lives in `ClusterUi`, above the panels and the
//! router, so hiding a panel or leaving the tab keeps it. Leadership, term,
//! and every index belong to the selected group: nothing here names a
//! cluster-wide leader.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::models::cluster_raft::{RaftGroup, RaftGroups};
use crate::models::format::clock_second;
use crate::routes::tabs::ClusterTab;
use crate::services::backend::Backend;
use crate::state::cluster::ClusterUi;
use crate::state::workspace::Surface;
use crate::views::cluster::raft_details::RaftDetails;
use crate::views::cluster::raft_review::TransferSheet;
use crate::views::events::common::{NoticeLine, StatusText};

const SURFACE: Surface = Surface::Cluster(ClusterTab::RaftGroups);
const GROUPS_SIZE: PanelSize = PanelSize::width(470.0, 340.0, 760.0);

/// Groups whose id, type, or leader contains `filter`, ignoring case.
fn matching<'a>(groups: &'a [RaftGroup], filter: &str) -> Vec<&'a RaftGroup> {
    let needle = filter.trim().to_lowercase();
    groups
        .iter()
        .filter(|g| {
            needle.is_empty()
                || [g.id.as_str(), g.kind.label(), g.leader_name()]
                    .iter()
                    .any(|f| f.to_lowercase().contains(&needle))
        })
        .collect()
}

/// The group to show: the chosen one while it exists, else the first group
/// that needs attention, else the first.
pub fn effective<'a>(groups: &'a RaftGroups, chosen: Option<&str>) -> Option<&'a RaftGroup> {
    chosen
        .and_then(|id| groups.group(id))
        .or_else(|| groups.default_group())
}

#[component]
pub fn RaftTab() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut ui = use_context::<ClusterUi>().raft;
    // Only a write bumps the epoch: a memo keeps typing in the filter from
    // re-running the read.
    let epoch = use_memo(move || ui.read().epoch);

    let mut groups = use_resource(move || {
        let backend = backend.clone();
        let _conn = conn();
        let _epoch = epoch();
        async move { backend.raft_groups().await }
    });
    let state = resource_state(&groups);
    let list = state.loaded().cloned();

    let (filter, notice, review, chosen) = {
        let u = ui.read();
        (
            u.filter.clone(),
            u.notice.clone(),
            u.review.clone(),
            u.group.clone(),
        )
    };
    let selected = list
        .as_ref()
        .and_then(|l| effective(l, chosen.as_deref()).cloned());
    let observed = list
        .as_ref()
        .map(|l| format!("Observed {} UTC", clock_second(l.observed_at, 0)))
        .unwrap_or_default();

    rsx! {
        div { class: "page-body",
            div { class: "workbench events-wb",
                Panel {
                    surface: SURFACE,
                    id: "groups",
                    title: "Raft groups",
                    size: GROUPS_SIZE,
                    actions: rsx! {
                        if !observed.is_empty() {
                            span { "{observed}" }
                        }
                    },
                    div { class: "toolbar nowrap",
                        div { class: "search-field grow",
                            Ico { icon: Icon::Search, size: 16 }
                            input {
                                class: "input",
                                placeholder: "Filter Raft groups…",
                                "aria-label": "Filter Raft groups",
                                value: "{filter}",
                                oninput: move |e| ui.write().filter = e.value(),
                            }
                        }
                        button {
                            class: "btn",
                            title: "Read the groups again",
                            onclick: move |_| groups.restart(),
                            Ico { icon: Icon::RefreshCw, size: 16 }
                            "Refresh"
                        }
                    }
                    AsyncView {
                        loading: state.is_loading(),
                        empty: state.is_empty(),
                        error: state.error_message(),
                        retriable: state.is_retriable(),
                        on_retry: move |_| groups.restart(),
                        empty_message: "This cluster reports no Raft group.",
                    }
                    if let Some(l) = list.as_ref() {
                        {
                            let rows = matching(&l.groups, &filter);
                            rsx! {
                                div { class: "dt-wrap",
                                    table { class: "dt cl-table",
                                        thead {
                                            tr {
                                                th { "Group" }
                                                th { "Leader" }
                                                th { class: "num", "Voters" }
                                                th { class: "num", "Reachable" }
                                                th { "State" }
                                            }
                                        }
                                        tbody {
                                            for g in rows.iter() {
                                                {
                                                    let id = g.id.clone();
                                                    let is_selected = selected.as_ref().is_some_and(|s| s.id == g.id);
                                                    let health = g.health();
                                                    rsx! {
                                                        tr {
                                                            key: "{g.id}",
                                                            class: if is_selected { "clickable selected" } else { "clickable" },
                                                            "aria-selected": "{is_selected}",
                                                            tabindex: "0",
                                                            onclick: move |_| {
                                                                let mut w = ui.write();
                                                                w.group = Some(id.clone());
                                                                w.target = None;
                                                            },
                                                            td { class: "key", "{g.id}" }
                                                            td { "{g.leader_name()}" }
                                                            td { class: "num", "{g.voter_count()}" }
                                                            td { class: "num", "{g.reachable_voters()} of {g.voter_count()}" }
                                                            td { StatusText { tone: health.tone(), text: health.label().to_string() } }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    if rows.is_empty() {
                                        div { class: "async-empty", "No group matches the filter." }
                                    }
                                }
                                div { class: "dt-footer",
                                    "{l.groups.len()} groups · each has its own leader, term, and quorum"
                                }
                            }
                        }
                    }
                }
                ResizeHandle { surface: SURFACE, target: "groups", size: GROUPS_SIZE, side: Side::After }
                Panel {
                    surface: SURFACE,
                    id: "details",
                    title: "Raft group details",
                    class: "fill",
                    NoticeLine { notice, on_dismiss: move |_| ui.write().notice = None }
                    if let Some(g) = selected {
                        RaftDetails { group: g }
                    } else if !state.is_loading() {
                        div { class: "async-empty", "Select a Raft group to see its peers." }
                    }
                }
                if let Some(r) = review {
                    TransferSheet { review: r }
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
    async fn filter_matches_id_type_and_leader() {
        let svc = MockConnectionService::ready();
        let all = svc.raft_groups().await.expect("groups");
        assert_eq!(matching(&all.groups, "").len(), all.groups.len());
        assert_eq!(matching(&all.groups, "DATA-1").len(), 3);
        assert_eq!(matching(&all.groups, "sequencer").len(), 1);
        assert_eq!(matching(&all.groups, "node-3").len(), 2);
        assert!(matching(&all.groups, "nope").is_empty());
    }

    #[tokio::test]
    async fn the_default_group_is_the_first_that_needs_attention() {
        let svc = MockConnectionService::ready();
        let all = svc.raft_groups().await.expect("groups");
        assert_eq!(
            effective(&all, None).map(|g| g.id.as_str()),
            Some("data-12")
        );
        assert_eq!(
            effective(&all, Some("sequencer-0")).map(|g| g.id.as_str()),
            Some("sequencer-0")
        );
        // A group that no longer exists falls back, never shows stale.
        assert_eq!(
            effective(&all, Some("data-99")).map(|g| g.id.as_str()),
            Some("data-12")
        );
    }
}
