//! Sync → Activity: reconnects, bootstrap stages, acknowledgments, rejects,
//! and administrative changes, newest first.
//!
//! Two panels. Time range, device, and event type filter on the server. The
//! search text filters the loaded rows. The log is the Origin's current
//! observation, not a global audit guarantee.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::models::cluster_activity::ActivityRange;
use crate::models::format::clock_second;
use crate::models::sync_activity::{SyncEventKind, stamp};
use crate::routes::tabs::SyncTab;
use crate::services::backend::Backend;
use crate::state::sync::{SyncUi, matching_events, pick_event};
use crate::state::workspace::Surface;
use crate::views::cluster::common::Ctl;
use crate::views::events::common::StatusText;
use crate::views::sync::activity_details::SyncEventDetails;
use crate::views::sync::common::dash;

const SURFACE: Surface = Surface::Sync(SyncTab::Activity);
const DETAILS_SIZE: PanelSize = PanelSize::width(340.0, 300.0, 560.0);

#[component]
pub fn ActivityTab() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut ui = use_context::<SyncUi>().activity;
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
            async move { backend.sync_activity(&query).await }
        }
    });
    // The device menu lists every registered device, not only those in rows.
    let devices = use_resource(move || {
        let backend = backend.clone();
        let _conn = conn();
        async move { backend.sync_devices().await }
    });

    let state = resource_state(&log);
    let data = state.loaded().cloned();
    let u = ui.read().clone();
    let device_list: Vec<(String, String)> = devices
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .map(|d| {
            d.devices
                .iter()
                .map(|x| (x.id.clone(), x.name.clone()))
                .collect()
        })
        .unwrap_or_default();
    let rows = data
        .as_ref()
        .map(|l| matching_events(&l.events, &u.text))
        .unwrap_or_default();
    let selected = pick_event(&rows, u.selected.as_deref()).cloned();
    let observed = data.as_ref().map(|l| l.observed_at);

    rsx! {
        div { class: "page-body",
            div { class: "cl-bar sy-activity-bar",
                Ctl { label: "Origin", id: "act-origin",
                    select {
                        id: "act-origin",
                        class: "select native",
                        title: "Sync is bound to one Origin cluster. Switch clusters from the connection menu.",
                        option { selected: true, "staging-cluster" }
                    }
                }
                Ctl { label: "Device", id: "act-device",
                    select {
                        id: "act-device",
                        class: "select native",
                        onchange: move |e| {
                            let v = e.value();
                            let mut w = ui.write();
                            w.device = (!v.is_empty()).then_some(v);
                            w.selected = None;
                        },
                        option { value: "", selected: u.device.is_none(), "All" }
                        for (id , name) in device_list.iter() {
                            option { key: "{id}", value: "{id}", selected: u.device.as_deref() == Some(id.as_str()), "{id} ({name})" }
                        }
                    }
                }
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
                Ctl { label: "Event", id: "act-kind",
                    select {
                        id: "act-kind",
                        class: "select native",
                        onchange: move |e| {
                            let v = e.value();
                            ui.write().kind = SyncEventKind::ALL.iter().copied().find(|k| k.label() == v);
                        },
                        option { value: "", selected: u.kind.is_none(), "All" }
                        for k in SyncEventKind::ALL {
                            option { key: "{k.label()}", value: "{k.label()}", selected: Some(k) == u.kind, "{k.label()}" }
                        }
                    }
                }
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
                button {
                    class: "icon-btn",
                    title: "Read the log now",
                    "aria-label": "Refresh activity",
                    onclick: move |_| log.restart(),
                    Ico { icon: Icon::RefreshCw, size: 16 }
                }
                button {
                    class: "btn primary",
                    disabled: true,
                    title: "The export format and destination are not designed yet",
                    Ico { icon: Icon::Download, size: 16 }
                    "Export"
                }
            }
            div { class: "workbench",
                Panel {
                    surface: SURFACE,
                    id: "activity",
                    title: "Sync activity",
                    class: "fill",
                    actions: rsx! {
                        if let Some(at) = observed {
                            span { "Observed {clock_second(at, 0)} UTC" }
                        }
                    },
                    AsyncView {
                        loading: state.is_loading(),
                        empty: state.is_empty(),
                        error: state.error_message(),
                        retriable: state.is_retriable(),
                        on_retry: move |_| log.restart(),
                        empty_message: "No sync event matches these filters in this time range.",
                    }
                    if let Some(l) = data.as_ref() {
                        div { class: "dt-wrap",
                            table { class: "dt cl-table",
                                thead {
                                    tr {
                                        th { "Time (UTC)" }
                                        th { "Event" }
                                        th { "Device" }
                                        th { "Shape" }
                                        th { "Direction" }
                                        th { "Details" }
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
                                                    onclick: move |_| ui.write().selected = Some(id.clone()),
                                                    td { class: "mono", "{stamp(e.at, l.observed_at)}" }
                                                    td { StatusText { tone: e.kind.tone(), text: e.kind.label().to_string() } }
                                                    td { class: "key", title: "{e.device_name}", "{e.device_id}" }
                                                    td {
                                                        match e.shape.as_ref() {
                                                            Some(s) => rsx! { "{s}" },
                                                            None => dash(),
                                                        }
                                                    }
                                                    td {
                                                        match e.direction {
                                                            Some(d) => rsx! { "{d.label()}" },
                                                            None => dash(),
                                                        }
                                                    }
                                                    td { class: "sy-detail",
                                                        match e.detail.as_ref() {
                                                            Some(d) => rsx! { span { title: "{d}", "{d}" } },
                                                            None => dash(),
                                                        }
                                                    }
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
                        div { class: "dt-footer", "{rows.len()} events" }
                    }
                }
                ResizeHandle { surface: SURFACE, target: "details", size: DETAILS_SIZE, side: Side::After }
                Panel {
                    surface: SURFACE,
                    id: "details",
                    title: "Event details",
                    size: DETAILS_SIZE,
                    if let Some(e) = selected {
                        SyncEventDetails { event: e }
                    } else if !state.is_loading() {
                        div { class: "async-empty", "Select an event to see its details." }
                    }
                }
            }
        }
    }
}
