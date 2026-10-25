//! Sync → Subscriptions: every shape the Origin sends to a device, the
//! synchronization status of the selected one, and its definition.
//!
//! Three panels: Subscriptions and Synchronization status stacked on the
//! left, Subscription details on the right. Changing a definition is a
//! review first: the Origin previews what it sends and removes, and apply
//! names the revision the preview read.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::models::format::clock_second;
use crate::models::sync_sub::Subscription;
use crate::routes::tabs::SyncTab;
use crate::services::backend::Backend;
use crate::state::sync::{SyncUi, matching_subs, pick_sub};
use crate::state::workspace::Surface;
use crate::views::cluster::common::Ctl;
use crate::views::events::common::{NoticeLine, StatusText};
use crate::views::sync::sub_details::SubDetails;
use crate::views::sync::sub_status::SubStatus;

const SURFACE: Surface = Surface::Sync(SyncTab::Subscriptions);
const STATUS_SIZE: PanelSize = PanelSize::height(330.0, 170.0, 560.0);
const DETAILS_SIZE: PanelSize = PanelSize::width(420.0, 330.0, 600.0);

#[component]
pub fn SubsTab() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut ui = use_context::<SyncUi>().subs;
    let epoch = use_memo(move || ui.read().epoch);

    let mut subs = use_resource(move || {
        let backend = backend.clone();
        let _conn = conn();
        let _epoch = epoch();
        async move { backend.sync_subscriptions().await }
    });
    let state = resource_state(&subs);
    let data = state.loaded().cloned();
    let u = ui.read().clone();
    let all: Vec<Subscription> = data.as_ref().map(|d| d.items.clone()).unwrap_or_default();
    let rows = matching_subs(&all, &u.filter, u.device.as_deref());
    let selected = pick_sub(&rows, u.selected.as_deref()).cloned();
    let observed = data.as_ref().map(|d| d.observed_at);
    // Every device that holds a subscription, once, in list order.
    let mut devices: Vec<(String, String)> = Vec::new();
    for s in &all {
        if !devices.iter().any(|(id, _)| *id == s.device_id) {
            devices.push((s.device_id.clone(), s.device_name.clone()));
        }
    }

    rsx! {
        div { class: "page-body",
            div { class: "workbench events-wb",
                div { class: "workbench col",
                    Panel {
                        surface: SURFACE,
                        id: "subscriptions",
                        title: "Subscriptions",
                        class: "fill",
                        actions: rsx! {
                            if let Some(at) = observed {
                                span { "Observed {clock_second(at, 0)} UTC" }
                            }
                        },
                        NoticeLine {
                            notice: u.notice.clone(),
                            on_dismiss: move |_| ui.write().notice = None,
                        }
                        div { class: "toolbar nowrap cl-filters",
                            Ctl { label: "Origin", id: "sub-origin",
                                select {
                                    id: "sub-origin",
                                    class: "select native",
                                    title: "Sync is bound to one Origin cluster. Switch clusters from the connection menu.",
                                    option { selected: true, "staging-cluster" }
                                }
                            }
                            Ctl { label: "Device", id: "sub-device",
                                select {
                                    id: "sub-device",
                                    class: "select native",
                                    onchange: move |e| {
                                        let v = e.value();
                                        let mut w = ui.write();
                                        w.device = (!v.is_empty()).then_some(v);
                                        w.selected = None;
                                    },
                                    option { value: "", selected: u.device.is_none(), "All devices" }
                                    for (id , name) in devices.iter() {
                                        option { key: "{id}", value: "{id}", selected: u.device.as_deref() == Some(id.as_str()), "{name}" }
                                    }
                                }
                            }
                            div { class: "search-field grow",
                                Ico { icon: Icon::Search, size: 16 }
                                input {
                                    class: "input",
                                    placeholder: "Filter subscriptions…",
                                    "aria-label": "Filter subscriptions",
                                    value: "{u.filter}",
                                    oninput: move |e| ui.write().filter = e.value(),
                                }
                            }
                            button {
                                class: "btn primary",
                                disabled: true,
                                title: "Creating a subscription needs a form that is not designed yet",
                                Ico { icon: Icon::Plus, size: 16 }
                                "Subscription"
                            }
                        }
                        AsyncView {
                            loading: state.is_loading(),
                            empty: state.is_empty(),
                            error: state.error_message(),
                            retriable: state.is_retriable(),
                            on_retry: move |_| subs.restart(),
                            empty_message: "No device has a subscription. A device with none receives no data.",
                        }
                        if data.is_some() {
                            div { class: "dt-wrap",
                                table { class: "dt cl-table",
                                    thead {
                                        tr {
                                            th { "Name" }
                                            th { "Device" }
                                            th { "Shape" }
                                            th { "Source" }
                                            th { "State" }
                                        }
                                    }
                                    tbody {
                                        for s in rows.iter() {
                                            {
                                                let id = s.id.clone();
                                                let is_selected = selected.as_ref().is_some_and(|x| x.id == s.id);
                                                rsx! {
                                                    tr {
                                                        key: "{s.id}",
                                                        class: if is_selected { "clickable selected" } else { "clickable" },
                                                        "aria-selected": "{is_selected}",
                                                        tabindex: "0",
                                                        onclick: move |_| ui.write().selected = Some(id.clone()),
                                                        td { class: "key", "{s.name}" }
                                                        td { "{s.device_id}" }
                                                        td { "{s.shape.label()}" }
                                                        td { "{s.source()}" }
                                                        td { StatusText { tone: s.state.tone(), text: s.state.label().to_string() } }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                if rows.is_empty() && !all.is_empty() {
                                    div { class: "async-empty", "No subscription matches these filters." }
                                }
                            }
                            div { class: "dt-footer", "{rows.len()} of {all.len()} subscriptions" }
                        }
                    }
                    ResizeHandle { surface: SURFACE, target: "status", size: STATUS_SIZE, side: Side::After }
                    Panel {
                        surface: SURFACE,
                        id: "status",
                        title: "Synchronization status",
                        size: STATUS_SIZE,
                        if let (Some(s), Some(at)) = (selected.clone(), observed) {
                            SubStatus { sub: s, observed_at: at }
                        }
                    }
                }
                ResizeHandle { surface: SURFACE, target: "details", size: DETAILS_SIZE, side: Side::After }
                Panel {
                    surface: SURFACE,
                    id: "details",
                    title: "Subscription details",
                    size: DETAILS_SIZE,
                    if let Some(s) = selected {
                        SubDetails { key: "{s.id}", sub: s }
                    }
                }
            }
        }
    }
}
