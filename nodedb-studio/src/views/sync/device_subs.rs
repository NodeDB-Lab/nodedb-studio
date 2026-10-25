//! The Subscribed data panel: what the selected device subscribes to, and
//! how many of its writes the Origin rejected.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::models::sync::Device;
use crate::routes::Route;
use crate::routes::tabs::SyncTab;
use crate::services::backend::Backend;
use crate::state::explorer::{RecordView, Selected, select_collection};
use crate::state::sync::SyncUi;
use crate::views::cluster::common::Ctl;
use crate::views::events::common::StatusText;

#[component]
pub fn DeviceSubs(device: Device, devices: Vec<Device>) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let nav = use_navigator();
    let ui = use_context::<SyncUi>();
    let (mut sel, mut subs_ui, mut rejects_ui) = (ui.devices, ui.subs, ui.rejects);
    let mut explorer = use_context::<Signal<Option<Selected>>>();
    let mut view = use_context::<Signal<RecordView>>();

    let mut subs = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            let _conn = conn();
            async move { backend.sync_subscriptions().await }
        }
    });
    let rejected = use_resource(move || {
        let backend = backend.clone();
        let _conn = conn();
        async move { backend.rejected_writes().await }
    });
    let state = resource_state(&subs);
    let mine: Vec<_> = state
        .loaded()
        .map(|s| {
            s.items
                .iter()
                .filter(|x| x.device_id == device.id)
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let open_rejects = rejected
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .map(|r| r.for_device(&device.id))
        .unwrap_or(0);
    let noun = if open_rejects == 1 { "write" } else { "writes" };
    let first_collection = mine.first().map(|s| s.collection.clone());
    let id = device.id.clone();
    let manage = {
        let id = id.clone();
        move |_| {
            {
                let mut w = subs_ui.write();
                w.device = Some(id.clone());
                w.selected = None;
            }
            nav.push(Route::Sync {
                tab: SyncTab::Subscriptions,
            });
        }
    };
    let review = {
        let id = id.clone();
        move |_| {
            {
                let mut w = rejects_ui.write();
                w.filter = id.clone();
                w.selected = None;
            }
            nav.push(Route::Sync {
                tab: SyncTab::RejectedWrites,
            });
        }
    };
    rsx! {
        div { class: "toolbar nowrap cl-filters",
            Ctl { label: "Device", id: "subs-device",
                select {
                    id: "subs-device",
                    class: "select native",
                    onchange: move |e| sel.write().selected = Some(e.value()),
                    for d in devices.iter() {
                        option { key: "{d.id}", value: "{d.id}", selected: d.id == id, "{d.name}" }
                    }
                }
            }
        }
        AsyncView {
            loading: state.is_loading(),
            empty: state.is_empty() || (state.loaded().is_some() && mine.is_empty()),
            error: state.error_message(),
            retriable: state.is_retriable(),
            on_retry: move |_| subs.restart(),
            empty_message: "This device has no subscription. It receives no data.",
        }
        if !mine.is_empty() {
            div { class: "dt-wrap",
                table { class: "dt cl-table",
                    thead {
                        tr {
                            th { "Database" }
                            th { "Collection" }
                            th { "Filter" }
                            th { "State" }
                        }
                    }
                    tbody {
                        for s in mine.iter() {
                            tr { key: "{s.id}",
                                td { "{s.database}" }
                                td { class: "key", "{s.collection}" }
                                td { class: "mono", "{s.resolved_filter()}" }
                                td { StatusText { tone: s.state.tone(), text: s.state.label().to_string() } }
                            }
                        }
                    }
                }
            }
        }
        div { class: "panel-footer sy-subs-actions",
            button { class: "btn", onclick: manage,
                Ico { icon: Icon::Settings, size: 16 }
                "Manage subscriptions"
            }
            button {
                class: "btn",
                disabled: first_collection.is_none(),
                onclick: move |_| {
                    if let Some(c) = first_collection.as_deref() {
                        select_collection(&mut explorer, &mut view, c);
                        nav.push(Route::Explorer {});
                    }
                },
                Ico { icon: Icon::FolderOpen, size: 16 }
                "Open in Explorer"
            }
        }
        if open_rejects > 0 {
            div { class: "callout warn sy-reject-bar",
                Ico { icon: Icon::TriangleAlert, size: 16 }
                span { "{open_rejects} rejected {noun}" }
                button { class: "link-btn", onclick: review, "Review rejected writes" }
            }
        }
    }
}
