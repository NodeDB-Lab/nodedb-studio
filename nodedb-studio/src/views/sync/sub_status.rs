//! The Synchronization status panel: the stage one subscription has
//! reached, and its recent events. Initial snapshot and live changes are
//! separate stages with separate times. The device reports received and
//! applied. The Origin holds the checkpoint.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::format::clock_second;
use crate::models::sync_sub::{Snapshot, Subscription};
use crate::routes::Route;
use crate::routes::tabs::SyncTab;
use crate::state::sync::SyncUi;
use crate::views::events::common::StatusText;
use crate::views::sync::common::unknown;

fn minute(at: Option<i64>) -> Element {
    match at {
        Some(t) => rsx! { span { class: "num", "{crate::models::format::clock(t, 0)}" } },
        None => unknown(),
    }
}

#[component]
pub fn SubStatus(sub: Subscription, observed_at: i64) -> Element {
    let nav = use_navigator();
    let ui = use_context::<SyncUi>();
    let (mut devices, mut activity) = (ui.devices, ui.activity);
    let s = &sub;
    let open_device = {
        let id = s.device_id.clone();
        move |_| {
            devices.write().selected = Some(id.clone());
            nav.push(Route::Sync {
                tab: SyncTab::Devices,
            });
        }
    };
    let view_all = {
        let id = s.device_id.clone();
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
    rsx! {
        div { class: "sy-status",
            div { class: "kv sy-status-facts",
                div { class: "kv-row", span { "Subscription" } span { class: "key", "{s.name}" } }
                div { class: "kv-row",
                    span { "Device" }
                    button { class: "link-btn", onclick: open_device, "{s.device_label()}" }
                }
                div { class: "kv-row", span { "Shape" } span { "{s.shape.label()}" } }
                div { class: "kv-row", span { "Source" } span { "{s.source()}" } }
                div { class: "kv-row", span { "Filter" } span { class: "mono", "{s.filter}" } }
                div { class: "kv-row",
                    span { "Parameters" }
                    span { class: "cl-chips",
                        for (name , value) in s.parameters.iter() {
                            span { key: "{name}", class: "chip", "{name} : {value}" }
                        }
                    }
                }
                div { class: "kv-row", span { "Permission scope" } span { "{s.permission_scope}" } }
                div { class: "kv-row",
                    span { "State" }
                    StatusText { tone: s.state.tone(), text: s.state.label().to_string() }
                }
                div { class: "kv-row", span { "Initial snapshot" } span { "{s.snapshot.label()}" } }
                div { class: "kv-row",
                    span { "Checkpoint" }
                    match s.checkpoint {
                        Some(c) => rsx! { span { class: "num", "{c}" } },
                        None => unknown(),
                    }
                }
                div { class: "kv-row", span { "Device last received" } {minute(s.last_received)} }
                div { class: "kv-row", span { "Device last applied" } {minute(s.last_applied)} }
                div { class: "kv-row", span { "Origin" } span { "staging-cluster" } }
                p { class: "inspector-note",
                    "Received and applied come from the device. The checkpoint is the Origin's. Observed {clock_second(observed_at, 0)} UTC."
                }
            }
            div { class: "sy-status-events",
                div { class: "sy-events-head",
                    h4 { class: "inspector-section", "Recent events" }
                    button { class: "link-btn", onclick: view_all,
                        "View all"
                        Ico { icon: Icon::ExternalLink, size: 16 }
                    }
                }
                div { class: "dt-wrap",
                    table { class: "dt cl-table sy-events",
                        thead {
                            tr {
                                th { class: "sy-time", "Time" }
                                th { "Event" }
                            }
                        }
                        tbody {
                            for e in s.events.iter() {
                                tr { key: "{e.at}-{e.kind:?}",
                                    td { class: "num", "{e.clock()}" }
                                    td { class: "sy-event-cell", title: "{e.text()}",
                                        span { class: "status",
                                            span { class: e.tone().dot() }
                                            span { class: "truncate", "{e.text()}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if matches!(s.snapshot, Snapshot::Running { .. }) {
            div { class: "callout warn sy-snapshot",
                Ico { icon: Icon::TriangleAlert, size: 16 }
                span { "Bootstrapping: live changes start after the snapshot completes. The device holds partial data until then." }
            }
        }
    }
}
