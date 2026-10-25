//! Sync → Devices: the Lite devices the Origin knows, what the selected one
//! subscribes to, and its details.
//!
//! Three panels: Devices and Subscribed data stacked on the left, Device
//! details on the right. The selection lives in `SyncUi`, above the panels,
//! so hiding a panel through the View menu keeps it. Every row shows when
//! the Origin collected it. A stage nobody reported reads Unknown.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::models::format::{ago, clock_second};
use crate::models::sync::{Device, DeviceState};
use crate::routes::tabs::SyncTab;
use crate::services::backend::Backend;
use crate::state::sync::{SyncUi, matching_devices, pick_device};
use crate::state::workspace::Surface;
use crate::views::cluster::common::Ctl;
use crate::views::events::common::{NoticeLine, StatusText};
use crate::views::sync::common::unknown;
use crate::views::sync::device_details::DeviceDetails;
use crate::views::sync::device_subs::DeviceSubs;
use crate::views::sync::enroll::EnrollSheet;
use crate::views::sync::enroll_tasks;

const SURFACE: Surface = Surface::Sync(SyncTab::Devices);
const SUBS_SIZE: PanelSize = PanelSize::height(260.0, 150.0, 520.0);
const DETAILS_SIZE: PanelSize = PanelSize::width(370.0, 300.0, 560.0);

#[component]
pub fn DevicesTab() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut ui = use_context::<SyncUi>().devices;
    let epoch = use_memo(move || ui.read().epoch);

    let mut devices = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            // Devices belong to the connection: a switch re-reads them.
            let _conn = conn();
            let _epoch = epoch();
            async move { backend.sync_devices().await }
        }
    });
    let state = resource_state(&devices);
    let data = state.loaded().cloned();
    let u = ui.read().clone();
    let all: Vec<Device> = data.as_ref().map(|d| d.devices.clone()).unwrap_or_default();
    let rows = matching_devices(&all, &u.filter, u.state);
    let selected = pick_device(&all, u.selected.as_deref()).cloned();
    let observed = data.as_ref().map(|d| d.observed_at);
    let register = {
        let backend = backend.clone();
        move |_| {
            let backend = backend.clone();
            spawn_forever(async move { enroll_tasks::open(ui, backend).await });
        }
    };

    rsx! {
        div { class: "page-body",
            div { class: "cl-bar",
                span { class: "cl-observed", "Origin cluster" }
                if let Some(d) = data.as_ref() {
                    span { class: "sy-origin", title: "The cluster id stops a rename from rebinding devices.",
                        "{d.origin.name} · {d.origin.cluster_id}"
                    }
                }
                span {
                    class: "chip",
                    title: "Sync spans every database. The database in the header does not narrow it.",
                    "Scope · Cluster-wide, all databases"
                }
                div { class: "toolbar-spacer" }
                button { class: "btn", onclick: move |_| devices.restart(),
                    Ico { icon: Icon::RefreshCw, size: 16 }
                    "Refresh"
                }
                button { class: "btn", onclick: register,
                    Ico { icon: Icon::Plus, size: 16 }
                    "Register device"
                }
            }
            div { class: "workbench events-wb",
                div { class: "workbench col",
                    Panel {
                        surface: SURFACE,
                        id: "devices",
                        title: "Devices",
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
                            div { class: "search-field grow",
                                Ico { icon: Icon::Search, size: 16 }
                                input {
                                    class: "input",
                                    placeholder: "Filter devices…",
                                    "aria-label": "Filter devices",
                                    value: "{u.filter}",
                                    oninput: move |e| ui.write().filter = e.value(),
                                }
                            }
                            Ctl { label: "State", id: "dev-state",
                                select {
                                    id: "dev-state",
                                    class: "select native",
                                    onchange: move |e| {
                                        let v = e.value();
                                        ui.write().state = DeviceState::ALL.iter().copied().find(|s| s.label() == v);
                                    },
                                    option { value: "", selected: u.state.is_none(), "All" }
                                    for s in DeviceState::ALL {
                                        option { key: "{s.label()}", value: "{s.label()}", selected: Some(s) == u.state, "{s.label()}" }
                                    }
                                }
                            }
                        }
                        AsyncView {
                            loading: state.is_loading(),
                            empty: state.is_empty(),
                            error: state.error_message(),
                            retriable: state.is_retriable(),
                            on_retry: move |_| devices.restart(),
                            empty_message: "No Lite device is registered with this Origin. Register a device to start syncing.",
                        }
                        if let Some(d) = data.as_ref() {
                            div { class: "dt-wrap",
                                table { class: "dt cl-table",
                                    thead {
                                        tr {
                                            th { "Device" }
                                            th { "Connection" }
                                            th { "Last contact" }
                                            th { "Upload queue" }
                                        }
                                    }
                                    tbody {
                                        for dev in rows.iter() {
                                            {
                                                let id = dev.id.clone();
                                                let is_selected = selected.as_ref().is_some_and(|s| s.id == dev.id);
                                                let contact = ago(d.observed_at.saturating_sub(dev.last_contact).max(0) as u64);
                                                rsx! {
                                                    tr {
                                                        key: "{dev.id}",
                                                        class: if is_selected { "clickable selected" } else { "clickable" },
                                                        "aria-selected": "{is_selected}",
                                                        tabindex: "0",
                                                        onclick: move |_| ui.write().selected = Some(id.clone()),
                                                        td { class: "key", "{dev.name}" }
                                                        td { StatusText { tone: dev.state.tone(), text: dev.state.label().to_string() } }
                                                        td { "{contact}" }
                                                        td {
                                                            match dev.pending {
                                                                Some(p) => rsx! {
                                                                    "{p.count} · reported {ago(d.observed_at.saturating_sub(p.reported_at).max(0) as u64)}"
                                                                },
                                                                None => unknown(),
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                if rows.is_empty() && !all.is_empty() {
                                    div { class: "async-empty", "No device matches these filters." }
                                }
                            }
                            div { class: "dt-footer",
                                "{all.len()} registered devices"
                                " · Queue sizes come from device reports."
                            }
                        }
                    }
                    ResizeHandle { surface: SURFACE, target: "subscribed", size: SUBS_SIZE, side: Side::After }
                    Panel {
                        surface: SURFACE,
                        id: "subscribed",
                        title: "Subscribed data",
                        size: SUBS_SIZE,
                        if let Some(dev) = selected.clone() {
                            DeviceSubs { device: dev, devices: all.clone() }
                        }
                    }
                }
                ResizeHandle { surface: SURFACE, target: "details", size: DETAILS_SIZE, side: Side::After }
                Panel {
                    surface: SURFACE,
                    id: "details",
                    title: "Device details",
                    size: DETAILS_SIZE,
                    if let (Some(dev), Some(d)) = (selected.clone(), data.as_ref()) {
                        DeviceDetails { device: dev, origin: d.origin.clone() }
                    }
                }
                if let Some(draft) = u.enroll.clone() {
                    EnrollSheet { draft }
                }
            }
        }
    }
}
