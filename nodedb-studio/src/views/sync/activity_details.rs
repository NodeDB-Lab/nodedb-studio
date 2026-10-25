//! The details of one sync event. A stage the event does not carry shows a
//! dash, and an acknowledgment nobody reported says so. Received, applied,
//! and acknowledged are three facts, never one.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::sync_activity::{Cursor, SyncEvent, SyncEventKind, utc};
use crate::routes::Route;
use crate::routes::tabs::SyncTab;
use crate::state::sync::SyncUi;
use crate::views::events::common::StatusText;
use crate::views::sync::common::dash;

fn cursor(c: Option<Cursor>) -> (Element, Element) {
    match c {
        Some(c) => (
            rsx! { span { class: "num", "{c.position}" } },
            rsx! { span { class: "num", "{utc(c.at)}" } },
        ),
        None => (dash(), dash()),
    }
}

#[component]
pub fn SyncEventDetails(event: SyncEvent) -> Element {
    let nav = use_navigator();
    let ui = use_context::<SyncUi>();
    let (mut devices, mut subs, mut rejects) = (ui.devices, ui.subs, ui.rejects);
    let e = &event;
    let (received_cursor, received_at) = cursor(e.received);
    let (applied_cursor, applied_at) = cursor(e.applied);
    let batch = e.kind == SyncEventKind::BatchApplied;
    let open_device = {
        let id = e.device_id.clone();
        move |_| {
            devices.write().selected = Some(id.clone());
            nav.push(Route::Sync {
                tab: SyncTab::Devices,
            });
        }
    };
    let open_sub = {
        let (device, shape) = (e.device_id.clone(), e.shape.clone());
        move |_| {
            {
                let mut w = subs.write();
                w.device = Some(device.clone());
                w.selected = None;
                if let Some(s) = shape.as_ref() {
                    w.filter = s.clone();
                }
            }
            nav.push(Route::Sync {
                tab: SyncTab::Subscriptions,
            });
        }
    };
    let open_reject = {
        let m = e.mutation.clone();
        move |_| {
            if let Some(m) = m.as_ref() {
                rejects.write().selected = Some(m.clone());
            }
            nav.push(Route::Sync {
                tab: SyncTab::RejectedWrites,
            });
        }
    };
    rsx! {
        div { class: "inspector cl-event",
            div { class: "kv",
                div { class: "kv-row",
                    span { "Event" }
                    StatusText { tone: e.kind.tone(), text: e.kind.label().to_string() }
                }
                div { class: "kv-row", span { "Device" } span { class: "mono", "{e.device_id}" } }
                div { class: "kv-row", span { "Device name" } span { "{e.device_name}" } }
                div { class: "kv-row", span { "Origin" } span { "staging-cluster" } }
                div { class: "kv-row", span { "Database" } span { "{e.database}" } }
                div { class: "kv-row",
                    span { "Shape" }
                    match e.shape.as_ref() {
                        Some(s) => rsx! { span { "{s}" } },
                        None => dash(),
                    }
                }
                div { class: "kv-row", span { "Event ID" } span { class: "mono", "{e.id}" } }
                div { class: "kv-row", span { "Observed" } span { class: "num", "{utc(e.at)}" } }
                if let Some(a) = e.actor.as_ref() {
                    div { class: "kv-row", span { "Requested by" } span { "{a}" } }
                }
            }
            if batch {
                div { class: "kv",
                    div { class: "kv-row", span { "Received cursor" } {received_cursor} }
                    div { class: "kv-row", span { "Received at" } {received_at} }
                    div { class: "kv-row", span { "Applied cursor" } {applied_cursor} }
                    div { class: "kv-row", span { "Applied at" } {applied_at} }
                    div { class: "kv-row",
                        span { "Acknowledgement" }
                        match e.acknowledged {
                            Some(c) => rsx! { span { class: "num", "cursor {c.position} at {utc(c.at)}" } },
                            None => rsx! { span { "Not reported" } },
                        }
                    }
                }
            } else if let Some(c) = e.acknowledged {
                div { class: "kv",
                    div { class: "kv-row",
                        span { "Acknowledgement" }
                        span { class: "num", "cursor {c.position} at {utc(c.at)}" }
                    }
                }
            }
            div { class: "kv",
                div { class: "kv-row",
                    span { "Direction" }
                    match e.direction {
                        Some(d) => rsx! { span { "{d.label()}" } },
                        None => dash(),
                    }
                }
                if let Some(d) = e.detail.as_ref() {
                    div { class: "kv-row", span { "Detail" } span { "{d}" } }
                }
            }
            div { class: "callout info",
                Ico { icon: Icon::Info, size: 16 }
                span { "Shows current observed state. Not a global audit guarantee." }
            }
            h4 { class: "inspector-section", "Links" }
            div { class: "cl-links sy-links",
                button { class: "link-btn", onclick: open_device,
                    "Open device"
                    Ico { icon: Icon::ExternalLink, size: 16 }
                }
                if e.shape.is_some() {
                    button { class: "link-btn", onclick: open_sub,
                        "Open subscription"
                        Ico { icon: Icon::ExternalLink, size: 16 }
                    }
                }
                if e.mutation.is_some() {
                    button { class: "link-btn", onclick: open_reject,
                        "Open rejected write"
                        Ico { icon: Icon::ExternalLink, size: 16 }
                    }
                }
            }
        }
    }
}
