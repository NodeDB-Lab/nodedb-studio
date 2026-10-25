//! The Device details inspector. Device to Origin and Origin to device are
//! separate sections. Each stage has its own time, and a stage nobody
//! reported reads Unknown. A pending count belongs to the device that
//! reported it.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::format::{ago, clock_second, date_second};
use crate::models::sync::{Device, DeviceState, OriginBinding};
use crate::routes::Route;
use crate::routes::tabs::SyncTab;
use crate::services::backend::Backend;
use crate::state::sync::SyncUi;
use crate::views::events::common::StatusBadge;
use crate::views::sync::common::{StageRow, dash, unknown};
use crate::views::sync::device_tasks;

fn changes(n: u32) -> String {
    format!("{n} {}", if n == 1 { "change" } else { "changes" })
}

#[component]
pub fn DeviceDetails(device: Device, origin: OriginBinding) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let nav = use_navigator();
    let ui = use_context::<SyncUi>();
    let (devices, mut activity) = (ui.devices, ui.activity);
    let mut menu = use_signal(|| false);
    let d = &device;
    let paused = d.state == DeviceState::Paused;
    let toggle = {
        let (id, revision) = (d.id.clone(), d.revision);
        move |_| {
            menu.set(false);
            let (backend, id) = (backend.clone(), id.clone());
            spawn_forever(async move {
                device_tasks::set_paused(devices, backend, id, !paused, revision).await;
            });
        }
    };
    let view_activity = {
        let id = d.id.clone();
        move |_| {
            {
                let mut w = activity.write();
                w.device = Some(id.clone());
                w.selected = None;
            }
            nav.push(Route::Sync {
                tab: SyncTab::Activity,
            });
        }
    };
    let observed = format!("{} UTC", clock_second(d.observed_at, 0));
    rsx! {
        div { class: "inspector sy-device",
            div { class: "inspector-head",
                h3 { class: "inspector-title", "{d.name}" }
                StatusBadge { tone: d.state.tone(), text: d.state.label().to_string() }
            }
            div { class: "kv",
                div { class: "kv-row", span { "Device ID" } span { class: "mono", "{d.id}" } }
                div { class: "kv-row", span { "Type" } span { "NodeDB-Lite" } }
                div { class: "kv-row", span { "Origin" } span { "{origin.name}" } }
                div { class: "kv-row", span { "Cluster ID" } span { class: "mono", "{origin.cluster_id}" } }
                div { class: "kv-row", span { "Principal" } span { class: "mono", "{d.principal}" } }
                div { class: "kv-row",
                    span { "Last contact" }
                    span { "{clock_second(d.last_contact, 0)} UTC" }
                }
                div { class: "kv-row",
                    span { "Configured Origin" }
                    span { class: "mono", "{origin.endpoint}" }
                }
                div { class: "kv-row",
                    span { "Observed transport" }
                    match d.transport.as_ref() {
                        Some(t) => rsx! { span { "{t}" } },
                        None => dash(),
                    }
                }
                div { class: "kv-row",
                    span { "Registered" }
                    span { "{date_second(d.registered_at, 0)}" }
                }
            }
            if let Some(f) = d.fault.as_ref() {
                div { class: "callout err", role: "alert",
                    Ico { icon: Icon::CircleAlert, size: 16 }
                    span { "{f}" }
                }
            }
            h4 { class: "inspector-section", "Device → Origin" }
            div { class: "kv",
                StageRow { label: "Last received", ms: d.upload.received_ms }
                StageRow { label: "Last applied", ms: d.upload.applied_ms }
                StageRow { label: "Last acknowledged", ms: d.upload.acknowledged_ms }
                div { class: "kv-row",
                    span { "Pending uploads" }
                    match d.pending {
                        Some(p) => rsx! {
                            span { class: "num", title: "Reported by the device at {clock_second(p.reported_at, 0)} UTC",
                                "{p.count} · device-reported"
                            }
                        },
                        None => unknown(),
                    }
                }
            }
            h4 { class: "inspector-section", "Origin → Device" }
            div { class: "kv",
                StageRow { label: "Last sent", ms: d.download.sent_ms }
                if d.download.device_received_ms.is_some() {
                    StageRow { label: "Device received", ms: d.download.device_received_ms }
                }
                if d.download.device_applied_ms.is_some() {
                    StageRow { label: "Device applied", ms: d.download.device_applied_ms }
                }
                StageRow { label: "Device acknowledged", ms: d.download.device_acknowledged_ms }
                div { class: "kv-row",
                    span { "Awaiting acknowledgment" }
                    match d.download.awaiting {
                        Some(n) => rsx! { span { class: "num", "{changes(n)}" } },
                        None => unknown(),
                    }
                }
            }
            p { class: "inspector-note", "{d.state.meaning()}" }
            p { class: "inspector-note sy-observed",
                if d.state == DeviceState::Connected {
                    "Observed {observed}"
                } else {
                    "Last observed {observed}, {ago(d.observed_at.saturating_sub(d.last_contact).max(0) as u64)} before contact was lost"
                }
            }
            div { class: "inspector-actions",
                button { class: "btn", onclick: view_activity,
                    Ico { icon: Icon::FileText, size: 16 }
                    "View activity"
                }
                div { class: "anchor",
                    button {
                        class: "btn icon",
                        "aria-label": "{d.name} actions",
                        "aria-haspopup": "menu",
                        onmousedown: move |e| e.stop_propagation(),
                        onclick: move |e| {
                            e.stop_propagation();
                            menu.toggle();
                        },
                        Ico { icon: Icon::Ellipsis, size: 16 }
                    }
                    if menu() {
                        div {
                            class: "popover align-right sy-menu",
                            role: "menu",
                            onmousedown: move |e| e.stop_propagation(),
                            onclick: move |e| e.stop_propagation(),
                            button {
                                class: "menu-item",
                                role: "menuitem",
                                title: if paused {
                                    "Resume sync for this device only. It reconnects first."
                                } else {
                                    "Pause sync for this device only. Other devices keep syncing."
                                },
                                onclick: toggle,
                                Ico { icon: if paused { Icon::Play } else { Icon::Pause }, size: 16 }
                                if paused {
                                    "Resume sync for {d.name}"
                                } else {
                                    "Pause sync for {d.name}"
                                }
                            }
                            div { class: "menu-sep" }
                            button {
                                class: "menu-item danger",
                                role: "menuitem",
                                disabled: true,
                                title: "Revoking access needs an impact review that is not designed yet",
                                Ico { icon: Icon::Ban, size: 16 }
                                "Revoke access…"
                            }
                        }
                    }
                }
            }
        }
    }
}
