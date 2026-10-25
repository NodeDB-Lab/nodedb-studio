//! The sheets over Rejected writes: edit and resubmit, review a discard,
//! and inspect where a repair stands. Nothing changes until the button in
//! the footer. None of them offers a winner between two versions of a
//! record: the Origin's check decided, and the repair follows its hint.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::components::json_editor::{JsonEditor, JsonState};
use crate::components::sheet::{Sheet, SheetFooter, SheetWidth};
use crate::models::format::clock_second;
use crate::models::json_text::json_issue;
use crate::models::sync_reject::Rejection;
use crate::services::backend::Backend;
use crate::state::sync::{RejectUi, RepairSheet, SyncUi};
use crate::views::events::common::StatusText;
use crate::views::sync::reject_tasks;

fn close(mut ui: Signal<RejectUi>) {
    ui.write().sheet = None;
}

#[component]
pub fn EditSheet(rejection: Rejection, text: String, busy: bool, error: Option<String>) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let mut ui = use_context::<SyncUi>().rejects;
    let r = &rejection;
    let mutation = r.mutation.clone();
    let invalid = json_issue(&text).is_some();
    let submit = {
        let collection = r.collection.clone();
        move |_| {
            let (backend, collection) = (backend.clone(), collection.clone());
            spawn_forever(async move { reject_tasks::resubmit(ui, backend, collection).await });
        }
    };
    rsx! {
        Sheet {
            title: format!("Edit and resubmit {mutation}"),
            width: SheetWidth::Medium,
            on_close: move |_| close(ui),
            footer: rsx! {
                SheetFooter {
                    error,
                    hint: Some(
                        format!(
                            "Creates a new write from {}. {mutation} stays on record with its payload.",
                            r.device_name,
                        ),
                    ),
                    button { class: "btn", disabled: busy, onclick: move |_| close(ui), "Cancel" }
                    button {
                        class: "btn primary",
                        disabled: busy || invalid,
                        onclick: submit,
                        "Resubmit as new write"
                    }
                }
            },
            div { class: "sheet-form",
                div { class: "callout warn",
                    Ico { icon: Icon::TriangleAlert, size: 16 }
                    span { "{r.reason.label()} on {r.constraint}. {r.hint.title()}: {r.hint.text()}" }
                }
                div { class: "kv",
                    div { class: "kv-row", span { "Direction" } span { "Device → Origin" } }
                    div { class: "kv-row", span { "Device" } span { "{r.device_label()}" } }
                    div { class: "kv-row", span { "Target" } span { "{r.source()}" } }
                }
                JsonEditor {
                    text: text.clone(),
                    label: "Payload of the new write",
                    on_input: move |v: String| {
                        if let Some(RepairSheet::Edit { text, error, .. }) = ui.write().sheet.as_mut() {
                            *text = v;
                            *error = None;
                        }
                    },
                }
                JsonState { text }
            }
        }
    }
}

#[component]
pub fn DiscardSheet(rejection: Rejection, busy: bool, error: Option<String>) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let ui = use_context::<SyncUi>().rejects;
    let r = &rejection;
    let discard = move |_| {
        let backend = backend.clone();
        spawn_forever(async move { reject_tasks::discard(ui, backend).await });
    };
    rsx! {
        Sheet {
            title: format!("Review discard of {}", r.mutation),
            width: SheetWidth::Narrow,
            on_close: move |_| close(ui),
            footer: rsx! {
                SheetFooter {
                    error,
                    hint: Some("This cannot be undone from Studio.".to_string()),
                    button { class: "btn", disabled: busy, onclick: move |_| close(ui), "Cancel" }
                    button {
                        class: "btn danger",
                        disabled: busy,
                        onclick: discard,
                        "Discard local pending write on {r.device_id}"
                    }
                }
            },
            div { class: "review-feed",
                div { class: "review-block",
                    h3 { class: "section-title", "Write" }
                    div { class: "kv",
                        div { class: "kv-row", span { "Mutation" } span { class: "mono", "{r.mutation}" } }
                        div { class: "kv-row", span { "Device" } span { "{r.device_label()}" } }
                        div { class: "kv-row", span { "Target" } span { "{r.source()}" } }
                        div { class: "kv-row", span { "Reason" } span { "{r.reason.label()}" } }
                    }
                }
                div { class: "review-block",
                    h3 { class: "section-title", "What changes" }
                    ul { class: "cl-effects",
                        li { "{r.device_name} drops its pending copy of {r.mutation} when it next reports." }
                        li { "The Origin keeps the rejected payload and this action in the audit history." }
                        li { "Other writes from {r.device_name} are not affected." }
                    }
                }
                div { class: "callout warn",
                    Ico { icon: Icon::TriangleAlert, size: 16 }
                    span { "The data in this write is not applied anywhere. Copy it from the details first if you need it." }
                }
            }
        }
    }
}

#[component]
pub fn CompensationSheet(rejection: Rejection) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let ui = use_context::<SyncUi>().rejects;
    let mutation = rejection.mutation.clone();
    let mut state = use_resource({
        let mutation = mutation.clone();
        move || {
            let backend = backend.clone();
            let _conn = conn();
            let _epoch = ui.read().epoch;
            let mutation = mutation.clone();
            async move { backend.compensation_state(&mutation).await }
        }
    });
    let read = resource_state(&state);
    let data = read.loaded().cloned();
    rsx! {
        Sheet {
            title: format!("Compensation state of {mutation}"),
            width: SheetWidth::Narrow,
            on_close: move |_| close(ui),
            footer: rsx! {
                SheetFooter { error: None, hint: None,
                    button { class: "btn primary", onclick: move |_| close(ui), "Close" }
                }
            },
            AsyncView {
                loading: read.is_loading(),
                empty: false,
                error: read.error_message(),
                retriable: read.is_retriable(),
                on_retry: move |_| state.restart(),
            }
            if let Some(c) = data {
                div { class: "review-feed",
                    ol { class: "sy-steps",
                        for step in c.steps.iter() {
                            li { key: "{step.label}",
                                div { class: "sy-step-head",
                                    StatusText { tone: step.state.tone(), text: step.state.label().to_string() }
                                    span { class: "sy-step-label", "{step.label}" }
                                    if let Some(at) = step.at {
                                        span { class: "muted num", "{clock_second(at, 0)} UTC" }
                                    }
                                }
                                if let Some(n) = step.note.as_ref() {
                                    p { class: "muted", "{n}" }
                                }
                            }
                        }
                    }
                    if let Some(n) = c.replacement.as_ref() {
                        div { class: "callout ok",
                            Ico { icon: Icon::CircleCheck, size: 16 }
                            span { "Replaced by write {n}. The rejected write stays on record." }
                        }
                    }
                    p { class: "inspector-note",
                        "Observed {clock_second(c.observed_at, 0)} UTC. A step the device has not reported stays Unknown."
                    }
                }
            }
        }
    }
}
