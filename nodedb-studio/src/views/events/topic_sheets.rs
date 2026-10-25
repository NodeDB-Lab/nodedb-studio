//! The topic sheets: Create topic, Publish, and the Reset offsets review.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::components::json_editor::{JsonEditor, JsonState};
use crate::components::sheet::{Sheet, SheetFooter, SheetWidth};
use crate::models::format::thousands;
use crate::models::topics::{ConsumerGroup, ResetEffect, ResetScope, ResetTarget, Topic};
use crate::services::backend::Backend;
use crate::state::events::EventsUi;
use crate::state::events::topics::{PublishSheet, ResetForm, TopicSheet};
use crate::views::events::common::FormField;
use crate::views::events::topic_tasks;

#[component]
pub fn CreateTopicSheet(sheet: TopicSheet, topics: Vec<Topic>) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let mut ui = use_context::<EventsUi>().topics;
    let form = sheet.form.clone();
    let issues = form.issues(&topics);
    let show = |field: &str| {
        issues
            .iter()
            .find(|(f, _)| *f == field)
            .map(|(_, m)| m.clone())
            .filter(|_| {
                sheet.tried || (field == "name" && !form.name.is_empty()) || field != "name"
            })
    };
    let (name_e, parts_e, ret_e) = (show("name"), show("partitions"), show("retention"));
    let name = form.name.trim().to_string();
    let mut edit = move |change: Box<dyn FnOnce(&mut TopicSheet)>| {
        if let Some(s) = ui.write().create.as_mut() {
            change(s);
            s.error = None;
        }
    };
    let create = {
        let issues = issues.clone();
        move |_| {
            if !issues.is_empty() {
                if let Some(s) = ui.write().create.as_mut() {
                    s.tried = true;
                }
                return;
            }
            let backend = backend.clone();
            spawn_forever(async move { topic_tasks::create_topic(ui, backend).await });
        }
    };
    rsx! {
        Sheet {
            title: "Create topic",
            width: SheetWidth::Narrow,
            on_close: move |_| ui.write().create = None,
            footer: rsx! {
                SheetFooter {
                    error: sheet.error.clone().or_else(|| (sheet.tried && !issues.is_empty()).then(|| format!("Fix {} issue(s) above to create the topic.", issues.len()))),
                    hint: Some("A topic is durable: its messages stay for the retention window and can be replayed.".to_string()),
                    button { class: "btn", onclick: move |_| ui.write().create = None, "Cancel" }
                    button {
                        class: "btn primary",
                        disabled: sheet.busy,
                        onclick: create,
                        if name.is_empty() { "Create topic" } else { "Create topic {name}" }
                    }
                }
            },
            div { class: "sheet-form",
                FormField { label: "Topic name", id: "ct-name", error: name_e.clone(),
                    input {
                        id: "ct-name",
                        class: if name_e.is_some() { "input invalid" } else { "input" },
                        value: "{form.name}",
                        placeholder: "audit_events",
                        spellcheck: "false",
                        autofocus: true,
                        "aria-invalid": "{name_e.is_some()}",
                        oninput: move |e| { let v = e.value(); edit(Box::new(move |s| s.form.name = v)); },
                    }
                }
                div { class: "field-row two",
                    FormField { label: "Partitions", id: "ct-partitions", error: parts_e.clone(),
                        input {
                            id: "ct-partitions",
                            class: if parts_e.is_some() { "input invalid" } else { "input" },
                            inputmode: "numeric",
                            value: "{form.partitions}",
                            oninput: move |e| { let v = e.value(); edit(Box::new(move |s| s.form.partitions = v)); },
                        }
                    }
                    FormField { label: "Retention (days)", id: "ct-retention", error: ret_e.clone(),
                        input {
                            id: "ct-retention",
                            class: if ret_e.is_some() { "input invalid" } else { "input" },
                            inputmode: "numeric",
                            value: "{form.retention_days}",
                            oninput: move |e| { let v = e.value(); edit(Box::new(move |s| s.form.retention_days = v)); },
                        }
                    }
                }
                div { class: "callout info",
                    Ico { icon: Icon::Info, size: 16 }
                    span { "Use a notification instead for a message that needs no replay. A notification reaches only the sessions listening when it is sent." }
                }
            }
        }
    }
}

#[component]
pub fn PublishSheetView(sheet: PublishSheet, topic: Option<Topic>, partitions: u32) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let mut ui = use_context::<EventsUi>().topics;
    let name = topic.map(|t| t.name).unwrap_or_default();
    let draft = sheet.draft.clone();
    let value_issue = draft.value_issue();
    let blocked = value_issue.is_some();
    rsx! {
        Sheet {
            title: "Publish to {name}",
            width: SheetWidth::Narrow,
            on_close: move |_| ui.write().publish = None,
            footer: rsx! {
                SheetFooter {
                    error: sheet.error.clone(),
                    hint: Some("The message is appended to the partition and kept for the retention window.".to_string()),
                    button { class: "btn", onclick: move |_| ui.write().publish = None, "Cancel" }
                    button {
                        class: "btn primary",
                        disabled: blocked || sheet.busy,
                        title: value_issue.clone().unwrap_or_default(),
                        onclick: move |_| {
                            let backend = backend.clone();
                            spawn_forever(async move { topic_tasks::publish(ui, backend).await });
                        },
                        Ico { icon: Icon::Send, size: 16 }
                        "Publish to {name}"
                    }
                }
            },
            div { class: "sheet-form",
                div { class: "field-row two",
                    FormField { label: "Partition", id: "pb-partition",
                        select {
                            id: "pb-partition",
                            class: "select native",
                            onchange: move |e| {
                                if let Some(s) = ui.write().publish.as_mut() {
                                    s.draft.partition = e.value().parse().ok();
                                }
                            },
                            option { value: "", selected: draft.partition.is_none(), "Automatic, by key" }
                            for p in 0..partitions {
                                option { key: "{p}", value: "{p}", selected: draft.partition == Some(p), "Partition {p}" }
                            }
                        }
                    }
                    FormField { label: "Key", id: "pb-key", hint: "Equal keys go to one partition, in order.".to_string(),
                        input {
                            id: "pb-key",
                            class: "input mono",
                            value: "{draft.key}",
                            spellcheck: "false",
                            oninput: move |e| {
                                if let Some(s) = ui.write().publish.as_mut() {
                                    s.draft.key = e.value();
                                    s.error = None;
                                }
                            },
                        }
                    }
                }
                div { class: "field",
                    span { class: "field-label", "Value (JSON)" }
                    div { class: "json-box",
                        JsonEditor {
                            text: draft.value.clone(),
                            label: "Message value",
                            on_input: move |t: String| {
                                if let Some(s) = ui.write().publish.as_mut() {
                                    s.draft.value = t;
                                    s.error = None;
                                }
                            },
                        }
                    }
                    JsonState { text: draft.value.clone() }
                }
            }
        }
    }
}

#[component]
pub fn ResetSheet(form: ResetForm, group: Option<ConsumerGroup>) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut ui = use_context::<EventsUi>().topics;
    let group = group.filter(|g| g.id == form.group);
    let name = group
        .as_ref()
        .map(|g| g.name.clone())
        .unwrap_or_else(|| form.group.clone());
    let partitions: Vec<u32> = group
        .as_ref()
        .map(|g| g.offsets.iter().map(|o| o.partition).collect())
        .unwrap_or_default();
    let target: Result<ResetTarget, String> = match form.target {
        ResetTarget::Earliest => Ok(ResetTarget::Earliest),
        ResetTarget::Latest => Ok(ResetTarget::Latest),
        ResetTarget::Offset(_) => form
            .offset
            .trim()
            .parse::<u64>()
            .map(ResetTarget::Offset)
            .map_err(|_| "Enter a whole number for the offset.".to_string()),
    };
    let scope = form.scope;

    let preview = {
        let backend = backend.clone();
        let key = (form.group.clone(), scope, target.clone().ok());
        use_resource(use_reactive!(|key| {
            let backend = backend.clone();
            let _conn = conn();
            async move {
                let (group, scope, target) = key;
                match target {
                    Some(t) => backend.preview_offset_reset(&group, scope, t).await,
                    None => Err(crate::services::error::StudioError::Rejected {
                        reason: "Enter the offset to preview the reset.".into(),
                    }),
                }
            }
        }))
    };
    let state = resource_state(&preview);
    let loaded = state.loaded().cloned();
    let can_apply =
        loaded.as_ref().is_some_and(|p| p.moves_anything()) && !form.busy && target.is_ok();
    let apply_target = target.clone().ok();

    let mut set_target = move |t: ResetTarget| {
        if let Some(r) = ui.write().reset.as_mut() {
            r.target = t;
            r.error = None;
        }
    };
    rsx! {
        Sheet {
            title: "Reset offsets — {name}",
            width: SheetWidth::Medium,
            on_close: move |_| ui.write().reset = None,
            footer: rsx! {
                SheetFooter {
                    error: form.error.clone(),
                    hint: Some("Nothing moves until you press the button on the right.".to_string()),
                    button { class: "btn", onclick: move |_| ui.write().reset = None, "Cancel" }
                    button {
                        class: "btn primary",
                        disabled: !can_apply,
                        title: if can_apply { "" } else { "The review shows no change to apply" },
                        onclick: move |_| {
                            let Some(t) = apply_target else { return };
                            let backend = backend.clone();
                            spawn_forever(async move { topic_tasks::apply_reset(ui, backend, scope, t).await });
                        },
                        "Reset offsets for {name}"
                    }
                }
            },
            div { class: "sheet-form",
                div { class: "field",
                    span { class: "field-label", id: "rs-scope", "Partitions to reset" }
                    div { class: "start-list", role: "radiogroup", "aria-labelledby": "rs-scope",
                        label { class: "start-option",
                            input {
                                r#type: "radio",
                                class: "radio",
                                name: "rs-scope",
                                checked: scope == ResetScope::AllPartitions,
                                onchange: move |_| {
                                    if let Some(r) = ui.write().reset.as_mut() { r.scope = ResetScope::AllPartitions; }
                                },
                            }
                            span { class: "start-text", span { class: "start-label", "All partitions" } }
                        }
                        label { class: "start-option",
                            input {
                                r#type: "radio",
                                class: "radio",
                                name: "rs-scope",
                                checked: matches!(scope, ResetScope::Partition(_)),
                                onchange: {
                                    let first = partitions.first().copied().unwrap_or(0);
                                    move |_| {
                                        if let Some(r) = ui.write().reset.as_mut() { r.scope = ResetScope::Partition(first); }
                                    }
                                },
                            }
                            span { class: "start-text", span { class: "start-label", "One partition" } }
                            select {
                                class: "select native compact",
                                "aria-label": "Partition",
                                disabled: !matches!(scope, ResetScope::Partition(_)),
                                onchange: move |e| {
                                    if let (Some(r), Ok(p)) = (ui.write().reset.as_mut(), e.value().parse::<u32>()) {
                                        r.scope = ResetScope::Partition(p);
                                    }
                                },
                                for p in partitions.iter().copied() {
                                    option { key: "{p}", value: "{p}", selected: scope == ResetScope::Partition(p), "Partition {p}" }
                                }
                            }
                        }
                    }
                }
                div { class: "field",
                    span { class: "field-label", id: "rs-target", "Move to" }
                    div { class: "start-list", role: "radiogroup", "aria-labelledby": "rs-target",
                        label { class: "start-option",
                            input {
                                r#type: "radio",
                                class: "radio",
                                name: "rs-target",
                                checked: form.target == ResetTarget::Earliest,
                                onchange: move |_| set_target(ResetTarget::Earliest),
                            }
                            span { class: "start-text",
                                span { class: "start-label", "Earliest retained offset" }
                                span { class: "hint", "Replay everything the topic still holds" }
                            }
                        }
                        label { class: "start-option",
                            input {
                                r#type: "radio",
                                class: "radio",
                                name: "rs-target",
                                checked: form.target == ResetTarget::Latest,
                                onchange: move |_| set_target(ResetTarget::Latest),
                            }
                            span { class: "start-text",
                                span { class: "start-label", "Latest offset" }
                                span { class: "hint", "Skip the records the group has not processed" }
                            }
                        }
                        label { class: "start-option",
                            input {
                                r#type: "radio",
                                class: "radio",
                                name: "rs-target",
                                checked: matches!(form.target, ResetTarget::Offset(_)),
                                onchange: {
                                    let n = form.offset.trim().parse().unwrap_or(0);
                                    move |_| set_target(ResetTarget::Offset(n))
                                },
                            }
                            span { class: "start-text",
                                span { class: "start-label", "Specific offset" }
                                span { class: "hint", "Replay from, or skip to, one offset" }
                            }
                            input {
                                class: if matches!(form.target, ResetTarget::Offset(_)) && target.is_err() { "input mono offset-input invalid" } else { "input mono offset-input" },
                                "aria-label": "Offset",
                                inputmode: "numeric",
                                disabled: !matches!(form.target, ResetTarget::Offset(_)),
                                value: "{form.offset}",
                                oninput: move |e| {
                                    if let Some(r) = ui.write().reset.as_mut() {
                                        r.offset = e.value();
                                        r.error = None;
                                    }
                                },
                            }
                        }
                    }
                    if let (ResetTarget::Offset(_), Err(m)) = (form.target, target.clone()) {
                        div { class: "error", role: "alert", "{m}" }
                    }
                }
                h3 { class: "section-title", "Review" }
                AsyncView {
                    loading: state.is_loading(),
                    empty: false,
                    error: state.error_message(),
                    retriable: false,
                }
                if let Some(p) = loaded {
                    div { class: "dt-wrap",
                        table { class: "dt",
                            thead {
                                tr {
                                    th { "Partition" }
                                    th { class: "num", "Current" }
                                    th { class: "num", "New" }
                                    th { "Effect" }
                                }
                            }
                            tbody {
                                for r in p.rows.iter() {
                                    tr { key: "{r.partition}",
                                        td { class: "mono", "{r.partition}" }
                                        td { class: "num mono", "{r.current}" }
                                        td { class: "num mono", "{r.new}" }
                                        td {
                                            span { class: match r.effect {
                                                ResetEffect::Replays(_) => "badge amber",
                                                ResetEffect::Skips(_) => "badge red",
                                                ResetEffect::Unchanged => "badge gray",
                                            }, "{r.effect.label()}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if p.replayed() > 0 {
                        div { class: "callout warn",
                            Ico { icon: Icon::TriangleAlert, size: 16 }
                            span { "{name} processes {thousands(p.replayed())} records again. Delivery is at least once, so side effects of those records repeat." }
                        }
                    }
                    if p.skipped() > 0 {
                        div { class: "callout err",
                            Ico { icon: Icon::CircleAlert, size: 16 }
                            span { "{name} never processes {thousands(p.skipped())} records. They stay in the topic until retention removes them." }
                        }
                    }
                    if !p.moves_anything() {
                        div { class: "insp-sub", "This choice leaves every committed offset where it is." }
                    }
                    div { class: "insp-sub", "Group members active: Not reported. Stop the group's consumers before a reset, or they commit over it." }
                }
            }
        }
    }
}
