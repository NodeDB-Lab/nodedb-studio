//! The details of one cluster event. A field the event does not carry shows
//! a dash, never a guess.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::components::json_view::JsonView;
use crate::models::cluster_activity::ClusterEvent;
use crate::models::format::clock_second;
use crate::routes::Route;
use crate::routes::tabs::ClusterTab;
use crate::state::cluster::ClusterUi;
use crate::views::cluster::common::NameChips;
use crate::views::events::common::StatusText;

fn dash() -> Element {
    rsx! { span { class: "muted", "—" } }
}

#[component]
pub fn EventDetails(event: ClusterEvent) -> Element {
    let nav = use_navigator();
    let ui = use_context::<ClusterUi>();
    let (mut raft, mut nodes, mut activity) = (ui.raft, ui.nodes, ui.activity);
    let raw_open = activity.read().raw_open;
    let e = &event;
    let raw = sonic_rs::to_string_pretty(e).unwrap_or_default();
    rsx! {
        div { class: "inspector cl-event",
            div { class: "kv",
                div { class: "kv-row", span { "Event ID" } span { class: "mono", "{e.id}" } }
                div { class: "kv-row", span { "Observed" } span { "{clock_second(e.observed_at, 0)} UTC" } }
                div { class: "kv-row", span { "Event time" } span { "{clock_second(e.event_time, 0)} UTC" } }
            }
            div { class: "kv",
                div { class: "kv-row", span { "Event" } span { "{e.kind.label()}" } }
                div { class: "kv-row",
                    span { "Severity" }
                    StatusText { tone: e.severity.tone(), text: e.severity.label().to_string() }
                }
            }
            p { class: "cl-scope-note", "{e.kind.scope_note()}" }
            div { class: "kv",
                div { class: "kv-row",
                    span { "Group" }
                    match e.group.as_ref() {
                        Some(g) => rsx! { span { class: "mono", "{g}" } },
                        None => dash(),
                    }
                }
                div { class: "kv-row",
                    span { "Term" }
                    match e.term {
                        Some(t) => rsx! { span { class: "num", "{t}" } },
                        None => dash(),
                    }
                }
                div { class: "kv-row",
                    span { "Previous leader" }
                    match e.previous_leader.as_ref() {
                        Some(p) => rsx! { span { "{p}" } },
                        None => dash(),
                    }
                }
                div { class: "kv-row",
                    span { "Leader" }
                    match e.leader.as_ref() {
                        Some(l) => rsx! { span { "{l}" } },
                        None => dash(),
                    }
                }
                div { class: "kv-row",
                    span { "Voters" }
                    if e.voters.is_empty() {
                        {dash()}
                    } else {
                        NameChips { names: e.voters.clone() }
                    }
                }
                div { class: "kv-row", span { "Node" } span { "{e.node}" } }
                div { class: "kv-row", span { "Source" } span { "{e.source}" } }
                div { class: "kv-row",
                    span { "Correlation" }
                    match e.correlation.as_ref() {
                        Some(c) => rsx! { span { class: "mono", "{c}" } },
                        None => dash(),
                    }
                }
                if let Some(a) = e.actor.as_ref() {
                    div { class: "kv-row", span { "Requested by" } span { "{a}" } }
                }
            }
            div { class: "cl-links",
                if let Some(g) = e.group.clone() {
                    button {
                        class: "link-btn",
                        onclick: move |_| {
                            {
                                let mut w = raft.write();
                                w.group = Some(g.clone());
                                w.target = None;
                            }
                            nav.push(Route::Cluster { tab: ClusterTab::RaftGroups });
                        },
                        "Open Raft group"
                        Ico { icon: Icon::ExternalLink, size: 16 }
                    }
                }
                button {
                    class: "link-btn",
                    onclick: {
                        let id = e.node_id.clone();
                        move |_| {
                            {
                                let mut w = nodes.write();
                                w.node = Some(id.clone());
                                w.group = None;
                            }
                            nav.push(Route::Cluster { tab: ClusterTab::Nodes });
                        }
                    },
                    "Open node"
                    Ico { icon: Icon::ExternalLink, size: 16 }
                }
            }
            button {
                class: "cl-raw-toggle",
                "aria-expanded": "{raw_open}",
                onclick: move |_| {
                    let open = activity.peek().raw_open;
                    activity.write().raw_open = !open;
                },
                Ico { icon: if raw_open { Icon::ChevronDown } else { Icon::ChevronRight }, size: 16 }
                "Raw event (JSON)"
            }
            if raw_open {
                JsonView { text: raw, class: "cl-raw" }
            }
        }
    }
}
