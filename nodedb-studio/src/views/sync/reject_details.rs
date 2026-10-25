//! The Write attempt details inspector: what the device sent, which check
//! the Origin failed, the record the Origin already holds, and the typed
//! repairs. The direction is always device to Origin.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::AsyncView;
use crate::components::icon::{Ico, Icon};
use crate::components::json_view::JsonView;
use crate::models::format::date_second;
use crate::models::record_json::pretty;
use crate::models::schema::SchemaField;
use crate::models::sync_reject::{RejectState, Rejection};
use crate::routes::Route;
use crate::services::backend::Backend;
use crate::state::explorer::{RecordView, Selected, select_collection};
use crate::state::sync::{RepairSheet, SyncUi};
use crate::views::events::common::StatusText;
use crate::views::sync::reject_tasks;

/// A collapsible block with a title and a document icon.
#[component]
fn Fold(title: String, open: bool, on_toggle: EventHandler<()>, children: Element) -> Element {
    rsx! {
        section { class: "sy-fold",
            button {
                class: "sy-fold-head",
                "aria-expanded": "{open}",
                onclick: move |_| on_toggle.call(()),
                Ico { icon: Icon::FileText, size: 16 }
                span { class: "sy-fold-title", "{title}" }
                Ico { icon: if open { Icon::ChevronUp } else { Icon::ChevronDown }, size: 16 }
            }
            if open {
                div { class: "sy-fold-body", {children} }
            }
        }
    }
}

#[component]
pub fn RejectDetails(rejection: Rejection) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let nav = use_navigator();
    let mut ui = use_context::<SyncUi>().rejects;
    let mut explorer = use_context::<Signal<Option<Selected>>>();
    let mut view = use_context::<Signal<RecordView>>();
    let mut payload_open = use_signal(|| true);
    let r = &rejection;
    let existing_open = ui.read().existing_open;

    // The record the Origin holds is the Explorer's record, read through
    // the same seam.
    let existing = use_resource({
        let backend = backend.clone();
        let (collection, key) = (r.collection.clone(), r.existing.clone());
        move || {
            let backend = backend.clone();
            let _conn = conn();
            let (collection, key) = (collection.clone(), key.clone());
            async move {
                let key = key?;
                let fields: Vec<SchemaField> = backend
                    .collection_schema(&collection)
                    .await
                    .map(|s| s.fields)
                    .unwrap_or_default();
                Some(
                    backend
                        .record(&collection, &key)
                        .await
                        .map(|rec| pretty(&rec.value, &fields)),
                )
            }
        }
    });
    let existing_read = existing.read().clone().flatten();
    let fields_for_payload = use_resource({
        let backend = backend.clone();
        let collection = r.collection.clone();
        move || {
            let backend = backend.clone();
            let collection = collection.clone();
            async move {
                backend
                    .collection_schema(&collection)
                    .await
                    .map(|s| s.fields)
                    .unwrap_or_default()
            }
        }
    });
    let payload_text = pretty(
        &r.payload,
        &fields_for_payload.read().clone().unwrap_or_default(),
    );

    let open = r.open();
    let blocked = r.retry_blocked();
    let (mutation, revision) = (r.mutation.clone(), r.revision);
    let retry = {
        let backend = backend.clone();
        let (mutation, revision) = (mutation.clone(), revision);
        move |_| {
            let (backend, mutation) = (backend.clone(), mutation.clone());
            spawn_forever(
                async move { reject_tasks::retry(ui, backend, mutation, revision).await },
            );
        }
    };
    let edit = {
        let backend = backend.clone();
        let r = rejection.clone();
        move |_| {
            let (backend, r) = (backend.clone(), r.clone());
            spawn_forever(async move { reject_tasks::open_edit(ui, backend, r).await });
        }
    };
    let discard = {
        let m = mutation.clone();
        move |_| {
            ui.write().sheet = Some(RepairSheet::Discard {
                mutation: m.clone(),
                revision,
                busy: false,
                error: None,
            });
        }
    };
    let inspect = {
        let m = mutation.clone();
        move |_| {
            ui.write().sheet = Some(RepairSheet::Compensation {
                mutation: m.clone(),
            });
        }
    };
    let open_existing = {
        let collection = r.collection.clone();
        move |_| {
            select_collection(&mut explorer, &mut view, &collection);
            nav.push(Route::Explorer {});
        }
    };
    rsx! {
        div { class: "inspector sy-reject",
            div { class: "kv",
                div { class: "kv-row", span { "Mutation" } span { class: "mono", "{r.mutation}" } }
                div { class: "kv-row", span { "Idempotency key" } span { class: "mono", "{r.idempotency_key}" } }
                div { class: "kv-row", span { "Origination device" } span { "{r.device_label()}" } }
                div { class: "kv-row", span { "Direction" } span { "Device → Origin" } }
                div { class: "kv-row", span { "Timestamp (device)" } span { class: "num", "{date_second(r.device_time, 0)}" } }
                div { class: "kv-row", span { "Received (origin)" } span { class: "num", "{date_second(r.received_at, 0)}" } }
                div { class: "kv-row", span { "Source" } span { "{r.source()}" } }
                div { class: "kv-row",
                    span { "State" }
                    StatusText { tone: r.state.tone(), text: r.state.label().to_string() }
                }
                div { class: "kv-row", span { "Reason" } span { class: "mono", "{r.reason.label()}" } }
            }
            div { class: "kv sy-constraint",
                div { class: "kv-row", span { "Origin constraint" } span { class: "mono", "{r.constraint}" } }
                div { class: "kv-row", span { "Compensation hint" } span { "{r.hint.title()}" } }
            }
            p { class: "inspector-note sy-hint", "{r.hint.text()}" }
            Fold {
                title: "Sanitized payload (device)",
                open: payload_open(),
                on_toggle: move |_| payload_open.toggle(),
                JsonView { text: payload_text }
            }
            if r.existing.is_some() {
                Fold {
                    title: "Authoritative existing record (origin)",
                    open: existing_open,
                    on_toggle: move |_| {
                        let now = ui.peek().existing_open;
                        ui.write().existing_open = !now;
                    },
                    AsyncView {
                        loading: existing_read.is_none(),
                        empty: false,
                        error: existing_read.as_ref().and_then(|x| x.as_ref().err()).map(|e| e.to_string()),
                        retriable: false,
                    }
                    if let Some(Ok(text)) = existing_read.clone() {
                        JsonView { text }
                        button { class: "link-btn", onclick: open_existing,
                            "Open {r.collection} in Explorer"
                            Ico { icon: Icon::ExternalLink, size: 16 }
                        }
                    }
                }
            }
            div { class: "sy-repairs",
                div { class: "sy-repair",
                    button {
                        class: "btn",
                        disabled: blocked.is_some(),
                        title: blocked.clone().unwrap_or_default(),
                        onclick: retry,
                        Ico { icon: Icon::RefreshCw, size: 16 }
                        "Retry unchanged"
                    }
                    if let Some(b) = blocked.as_ref() {
                        span { class: "sy-repair-note", "{b}" }
                    }
                }
                button {
                    class: "btn",
                    disabled: !open,
                    title: if open { "Correct the payload and submit it as a new write" } else { "Only a quarantined write can be edited" },
                    onclick: edit,
                    Ico { icon: Icon::Pencil, size: 16 }
                    "Edit and resubmit as new write"
                }
                button {
                    class: "btn",
                    disabled: !open,
                    title: if open { "Review dropping the device's pending copy" } else { "Only a quarantined write can be discarded" },
                    onclick: discard,
                    Ico { icon: Icon::Trash, size: 16 }
                    "Discard local pending write"
                }
            }
            button { class: "btn block", onclick: inspect,
                Ico { icon: Icon::Search, size: 16 }
                "Inspect compensation state"
            }
            if r.state != RejectState::Quarantined {
                p { class: "inspector-note", "This write is {r.state.label().to_lowercase()}. The rejected payload stays on record." }
            }
        }
    }
}
