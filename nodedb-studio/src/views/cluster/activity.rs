//! Cluster → Activity: membership changes, leadership changes, and the
//! history of administrative operations, newest first.
//!
//! Two panels. Range, severity, node, and group filter on the server. The
//! search text filters the loaded rows. Every row shows when the server
//! observed it. Nothing here states a cause the server did not report.

use std::rc::Rc;
use std::time::Duration;

use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::models::cluster_activity::{ActivityRange, Severity, stamp};
use crate::models::format::clock_second;
use crate::routes::tabs::ClusterTab;
use crate::services::backend::Backend;
use crate::state::cluster::{ClusterUi, matching, pick_event};
use crate::state::workspace::Surface;
use crate::views::cluster::activity_details::EventDetails;
use crate::views::cluster::common::Ctl;
use crate::views::events::common::StatusText;

const SURFACE: Surface = Surface::Cluster(ClusterTab::Activity);
const DETAILS_SIZE: PanelSize = PanelSize::width(380.0, 300.0, 560.0);
/// How often auto-refresh reads the log again.
const REFRESH: Duration = Duration::from_secs(15);

#[component]
pub fn ActivityTab() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut ui = use_context::<ClusterUi>().activity;
    let key = use_memo(move || {
        let u = ui.read();
        (u.query(), u.tick)
    });

    let mut log = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            let _conn = conn();
            let (query, _tick) = key();
            async move { backend.cluster_activity(&query).await }
        }
    });
    // The filter menus list every member and group, not only those in rows.
    let members = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            let _conn = conn();
            async move { backend.cluster_members().await }
        }
    });
    let groups = use_resource(move || {
        let backend = backend.clone();
        let _conn = conn();
        async move { backend.raft_groups().await }
    });
    // Auto-refresh reads the log on a timer. The loop stops with the tab.
    use_resource(move || {
        let on = ui.read().auto_refresh;
        async move {
            if !on {
                return;
            }
            loop {
                tokio::time::sleep(REFRESH).await;
                let next = ui.peek().tick + 1;
                ui.write().tick = next;
            }
        }
    });

    let state = resource_state(&log);
    let data = state.loaded().cloned();
    let u = ui.read().clone();
    let member_list: Vec<(String, String)> = members
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .map(|m| {
            m.members
                .iter()
                .map(|n| (n.id.clone(), n.name.clone()))
                .collect()
        })
        .unwrap_or_default();
    let group_list: Vec<String> = groups
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .map(|g| g.groups.iter().map(|x| x.id.clone()).collect())
        .unwrap_or_default();
    let rows = data
        .as_ref()
        .map(|l| matching(&l.events, &u.text))
        .unwrap_or_default();
    let selected = pick_event(&rows, u.selected.as_deref()).cloned();
    let observed = data.as_ref().map(|l| l.observed_at);

    rsx! {
        div { class: "page-body",
            div { class: "workbench",
                Panel {
                    surface: SURFACE,
                    id: "activity",
                    title: "Activity",
                    class: "fill",
                    div { class: "toolbar nowrap cl-filters",
                        Ctl { label: "Time range", id: "act-range",
                            select {
                                id: "act-range",
                                class: "select native",
                                onchange: move |e| {
                                    let v = e.value();
                                    if let Some(r) = ActivityRange::ALL.iter().find(|r| r.label() == v) {
                                        ui.write().range = *r;
                                    }
                                },
                                for r in ActivityRange::ALL {
                                    option { key: "{r.label()}", value: "{r.label()}", selected: r == u.range, "{r.label()}" }
                                }
                            }
                        }
                        Ctl { label: "Severity", id: "act-sev",
                            select {
                                id: "act-sev",
                                class: "select native",
                                onchange: move |e| {
                                    let v = e.value();
                                    ui.write().severity = Severity::ALL.iter().copied().find(|s| s.label() == v);
                                },
                                option { value: "", selected: u.severity.is_none(), "All" }
                                for s in Severity::ALL {
                                    option { key: "{s.label()}", value: "{s.label()}", selected: Some(s) == u.severity, "{s.label()}" }
                                }
                            }
                        }
                        Ctl { label: "Node", id: "act-node",
                            select {
                                id: "act-node",
                                class: "select native",
                                onchange: move |e| {
                                    let v = e.value();
                                    ui.write().node = (!v.is_empty()).then_some(v);
                                },
                                option { value: "", selected: u.node.is_none(), "All" }
                                for (id , name) in member_list.iter() {
                                    option { key: "{id}", value: "{id}", selected: u.node.as_deref() == Some(id.as_str()), "{name}" }
                                }
                            }
                        }
                        Ctl { label: "Group", id: "act-group",
                            select {
                                id: "act-group",
                                class: "select native",
                                onchange: move |e| {
                                    let v = e.value();
                                    ui.write().group = (!v.is_empty()).then_some(v);
                                },
                                option { value: "", selected: u.group.is_none(), "All" }
                                for g in group_list.iter() {
                                    option { key: "{g}", value: "{g}", selected: u.group.as_deref() == Some(g.as_str()), "{g}" }
                                }
                            }
                        }
                    }
                    div { class: "toolbar nowrap cl-filters",
                        div { class: "search-field grow",
                            Ico { icon: Icon::Search, size: 16 }
                            input {
                                class: "input",
                                placeholder: "Search events…",
                                "aria-label": "Search events",
                                value: "{u.text}",
                                oninput: move |e| ui.write().text = e.value(),
                            }
                        }
                        label { class: "cl-switch", title: "Read the log again every 15 seconds",
                            input {
                                r#type: "checkbox",
                                class: "switch",
                                checked: u.auto_refresh,
                                onchange: move |e| ui.write().auto_refresh = e.checked(),
                            }
                            "Auto-refresh"
                        }
                        button {
                            class: "icon-btn",
                            title: "Read the log now",
                            "aria-label": "Refresh activity",
                            onclick: move |_| log.restart(),
                            Ico { icon: Icon::RefreshCw, size: 16 }
                        }
                        button {
                            class: "btn",
                            disabled: true,
                            title: "The export format and destination are not designed yet",
                            Ico { icon: Icon::Download, size: 16 }
                            "Export"
                        }
                    }
                    AsyncView {
                        loading: state.is_loading(),
                        empty: state.is_empty(),
                        error: state.error_message(),
                        retriable: state.is_retriable(),
                        on_retry: move |_| log.restart(),
                        empty_message: "No event matches these filters in this time range.",
                    }
                    if let Some(l) = data.as_ref() {
                        div { class: "dt-wrap",
                            table { class: "dt cl-table",
                                thead {
                                    tr {
                                        th { "Timestamp (UTC)" }
                                        th { "Severity" }
                                        th { "Event" }
                                        th { "Group" }
                                        th { "Node" }
                                    }
                                }
                                tbody {
                                    for e in rows.iter() {
                                        {
                                            let id = e.id.clone();
                                            let is_selected = selected.as_ref().is_some_and(|s| s.id == e.id);
                                            rsx! {
                                                tr {
                                                    key: "{e.id}",
                                                    class: if is_selected { "clickable selected" } else { "clickable" },
                                                    "aria-selected": "{is_selected}",
                                                    tabindex: "0",
                                                    onclick: move |_| {
                                                        let mut w = ui.write();
                                                        w.selected = Some(id.clone());
                                                        w.raw_open = false;
                                                    },
                                                    td { class: "mono", "{stamp(e.observed_at, l.observed_at)}" }
                                                    td { StatusText { tone: e.severity.tone(), text: e.severity.label().to_string() } }
                                                    td { class: "key", "{e.kind.label()}" }
                                                    td {
                                                        match e.group.as_ref() {
                                                            Some(g) => rsx! { "{g}" },
                                                            None => rsx! { span { class: "muted", "—" } },
                                                        }
                                                    }
                                                    td { "{e.node}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            if rows.is_empty() && !l.events.is_empty() {
                                div { class: "async-empty", "No loaded event matches the search." }
                            }
                        }
                        div { class: "dt-footer",
                            "{rows.len()} events"
                            if let Some(at) = observed {
                                " · Last updated {clock_second(at, 0)} UTC"
                            }
                        }
                    }
                }
                ResizeHandle { surface: SURFACE, target: "details", size: DETAILS_SIZE, side: Side::After }
                Panel {
                    surface: SURFACE,
                    id: "details",
                    title: "Event details",
                    size: DETAILS_SIZE,
                    if let Some(e) = selected {
                        EventDetails { event: e }
                    } else if !state.is_loading() {
                        div { class: "async-empty", "Select an event to see its details." }
                    }
                }
            }
        }
    }
}
