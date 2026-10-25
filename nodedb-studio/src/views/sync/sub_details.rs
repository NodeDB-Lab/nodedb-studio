//! The Subscription details inspector: the definition, the estimated scope,
//! and the review of a change. An edit stays a draft until Apply. The draft
//! belongs to one subscription, so selecting another never carries it over.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::AsyncView;
use crate::components::clipboard::copy_text;
use crate::components::icon::{Ico, Icon};
use crate::components::json_view::JsonView;
use crate::components::tabs::TabBar;
use crate::models::format::thousands;
use crate::models::sync_sub::{ChangePreview, SubChange, SubState, Subscription, size_text};
use crate::services::backend::Backend;
use crate::state::sync::{SubDraft, SubTab, SubsUi, SyncUi};
use crate::views::events::common::{FormField, InfoTip};
use crate::views::sync::sub_tasks;

/// Apply `change` to the subscription's draft, starting one first.
fn edit(mut ui: Signal<SubsUi>, sub: &Subscription, change: impl FnOnce(&mut SubChange)) {
    let mut w = ui.write();
    if w.draft.as_ref().is_none_or(|d| d.id != sub.id) {
        w.draft = Some(SubDraft {
            id: sub.id.clone(),
            change: sub.as_change(),
        });
    }
    if let Some(d) = w.draft.as_mut() {
        change(&mut d.change);
    }
    w.error = None;
}

#[component]
pub fn SubDetails(sub: Subscription) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut ui = use_context::<SyncUi>().subs;
    let u = ui.read().clone();
    let change = u
        .draft_of(&sub.id)
        .cloned()
        .unwrap_or_else(|| sub.as_change());
    let dirty = u.dirty(&sub);
    let issues = change.issues();
    let tab = u.tab;
    let paused = sub.state == SubState::Paused;

    // The preview is read only on the tabs that show it.
    let mut preview = use_resource({
        let backend = backend.clone();
        let sub_id = sub.id.clone();
        move || {
            let backend = backend.clone();
            let _conn = conn();
            let id = sub_id.clone();
            let (tab, epoch) = {
                let u = ui.read();
                (u.tab, u.epoch)
            };
            let change = ui.read().draft_of(&id).cloned();
            async move {
                let _ = epoch;
                if tab == SubTab::Definition {
                    return None;
                }
                // The stored definition when nothing is edited.
                let current = match change {
                    Some(c) => c,
                    None => {
                        let subs = backend.sync_subscriptions().await.ok()?;
                        subs.get(&id)?.as_change()
                    }
                };
                Some(backend.preview_subscription_change(&id, &current).await)
            }
        }
    });
    let read = preview.read().clone();
    let pv: Option<Result<ChangePreview, _>> = read.flatten();
    let pv_state = match pv.clone() {
        Some(r) => crate::services::async_state::AsyncState::from_value(Some(r)),
        None => crate::services::async_state::AsyncState::Loading,
    };
    let loaded = pv_state.loaded().cloned();

    let json = sub.definition_json(Some(&change));
    let copy_json = {
        let json = json.clone();
        move |_| {
            let json = json.clone();
            spawn_forever(async move {
                let _ = copy_text(json).await;
            });
        }
    };
    let toggle_pause = {
        let backend = backend.clone();
        let (id, revision) = (sub.id.clone(), sub.revision);
        move |_| {
            let (backend, id) = (backend.clone(), id.clone());
            spawn_forever(async move {
                sub_tasks::set_paused(ui, backend, id, !paused, revision).await;
            });
        }
    };
    rsx! {
        div { class: "inspector sy-sub",
            TabBar {
                items: SubTab::ALL.iter().map(|t| t.label()).collect(),
                active: tab.index(),
                on_select: move |i: usize| {
                    if let Some(t) = SubTab::ALL.get(i) {
                        ui.write().tab = *t;
                    }
                },
            }
            match tab {
                SubTab::Definition => rsx! {
                    div { class: "sheet-form sy-form",
                        FormField { label: "Name", id: "sub-name",
                            input { id: "sub-name", class: "input", readonly: true, value: "{sub.name}" }
                        }
                        FormField { label: "Device", id: "sub-dev",
                            select { id: "sub-dev", class: "select native", disabled: true,
                                option { selected: true, "{sub.device_label()}" }
                            }
                        }
                        FormField { label: "Shape", id: "sub-shape",
                            div { class: "sy-inline",
                                select { id: "sub-shape", class: "select native", disabled: true,
                                    option { selected: true, "{sub.shape.label()}" }
                                }
                                InfoTip { text: sub.shape.hint().to_string() }
                            }
                        }
                        FormField { label: "Source collection", id: "sub-src",
                            select { id: "sub-src", class: "select native", disabled: true,
                                option { selected: true, "{sub.source()}" }
                            }
                        }
                        FormField {
                            label: "Filter",
                            id: "sub-filter",
                            error: issues.iter().find(|i| i.contains("filter")).cloned(),
                            input {
                                id: "sub-filter",
                                class: "input mono",
                                value: "{change.filter}",
                                oninput: {
                                    let sub = sub.clone();
                                    move |e| {
                                        let v = e.value();
                                        edit(ui, &sub, |c| c.filter = v);
                                    }
                                },
                            }
                        }
                        for (i , (name , value)) in change.parameters.iter().enumerate() {
                            FormField {
                                key: "{name}",
                                label: "Parameter",
                                id: "sub-param-{name}",
                                error: issues.iter().find(|m| m.contains(&format!(":{name}"))).cloned(),
                                div { class: "sy-param",
                                    input { class: "input mono", readonly: true, "aria-label": "Parameter name", value: "{name}" }
                                    input {
                                        id: "sub-param-{name}",
                                        class: "input mono",
                                        "aria-label": "Value of {name}",
                                        value: "{value}",
                                        oninput: {
                                            let sub = sub.clone();
                                            move |e| {
                                                let v = e.value();
                                                edit(ui, &sub, |c| {
                                                    if let Some(p) = c.parameters.get_mut(i) {
                                                        p.1 = v;
                                                    }
                                                });
                                            }
                                        },
                                    }
                                }
                            }
                        }
                        FormField {
                            label: "Permission scope",
                            id: "sub-scope",
                            hint: Some("Set by the device's role. A subscription cannot widen it.".to_string()),
                            select { id: "sub-scope", class: "select native", disabled: true,
                                option { selected: true, "{sub.permission_scope}" }
                            }
                        }
                        if let Some(p) = sub.preset.as_ref() {
                            div { class: "sy-includes",
                                div { class: "field-label",
                                    "Shape includes"
                                    InfoTip { text: "The preset this subscription came from.".to_string() }
                                }
                                select { class: "select native", disabled: true, "aria-label": "Preset",
                                    option { selected: true, "{p}" }
                                }
                            }
                        }
                        div { class: "sy-json",
                            div { class: "sy-json-head",
                                span { class: "field-label", "Definition (JSON)" }
                                button {
                                    class: "icon-btn sm",
                                    title: "Copy definition",
                                    "aria-label": "Copy definition",
                                    onclick: copy_json,
                                    Ico { icon: Icon::Copy, size: 16 }
                                }
                            }
                            JsonView { text: json.clone(), class: "sy-json-view" }
                        }
                    }
                },
                SubTab::EstimatedScope => rsx! {
                    div { class: "sy-pane",
                        AsyncView {
                            loading: pv_state.is_loading(),
                            empty: false,
                            error: pv_state.error_message(),
                            retriable: pv_state.is_retriable(),
                            on_retry: move |_| preview.restart(),
                        }
                        if let Some(p) = loaded.as_ref() {
                            table { class: "dt cl-table",
                                thead {
                                    tr {
                                        th { "" }
                                        th { "Rows" }
                                        th { "Size" }
                                    }
                                }
                                tbody {
                                    tr {
                                        td { class: "key", "Current definition" }
                                        td { class: "num", "{thousands(p.current.rows)}" }
                                        td { class: "num", "{size_text(p.current.bytes)}" }
                                    }
                                    if dirty {
                                        tr {
                                            td { class: "key", "Edited definition" }
                                            td { class: "num", "{thousands(p.proposed.rows)}" }
                                            td { class: "num", "{size_text(p.proposed.bytes)}" }
                                        }
                                    }
                                }
                            }
                            p { class: "inspector-note",
                                "The Origin estimates scope from its own statistics. The device holds fewer rows while it bootstraps."
                            }
                        }
                    }
                },
                SubTab::ReviewChange => rsx! {
                    div { class: "sy-pane",
                        if !dirty {
                            div { class: "async-empty", "Edit the filter or a parameter in Definition to review its impact." }
                        } else if !issues.is_empty() {
                            div { class: "callout err", role: "alert",
                                Ico { icon: Icon::CircleAlert, size: 16 }
                                span { "{issues[0]}" }
                            }
                        } else {
                            AsyncView {
                                loading: pv_state.is_loading(),
                                empty: false,
                                error: pv_state.error_message(),
                                retriable: pv_state.is_retriable(),
                                on_retry: move |_| preview.restart(),
                            }
                            if let Some(p) = loaded.as_ref() {
                                ReviewBody { preview: p.clone(), busy: u.busy, error: u.error.clone() }
                            }
                        }
                    }
                },
            }
            div { class: "inspector-actions",
                button {
                    class: "btn",
                    title: if paused {
                        "Resume this subscription. The device's other subscriptions are not affected."
                    } else {
                        "Pause this subscription. The device's other subscriptions keep syncing."
                    },
                    onclick: toggle_pause,
                    Ico { icon: if paused { Icon::Play } else { Icon::Pause }, size: 16 }
                    if paused {
                        "Resume subscription"
                    } else {
                        "Pause subscription"
                    }
                }
                if dirty {
                    button {
                        class: "btn",
                        onclick: move |_| ui.write().draft = None,
                        Ico { icon: Icon::RotateCcw, size: 16 }
                        "Reset edit"
                    }
                }
                if dirty && tab != SubTab::ReviewChange {
                    button {
                        class: "btn primary",
                        disabled: !issues.is_empty(),
                        title: if issues.is_empty() { "" } else { "Fix the definition first" },
                        onclick: move |_| ui.write().tab = SubTab::ReviewChange,
                        "Review change"
                    }
                }
            }
        }
    }
}

#[component]
fn ReviewBody(preview: ChangePreview, busy: bool, error: Option<String>) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let ui = use_context::<SyncUi>().subs;
    let p = preview.clone();
    let apply = move |_| {
        let (backend, p) = (backend.clone(), p.clone());
        spawn_forever(async move { sub_tasks::apply(ui, backend, p).await });
    };
    let granted = preview.permission.granted;
    rsx! {
        div { class: "kv",
            div { class: "kv-row", span { "Subscription" } span { class: "key", "{preview.name}" } }
            div { class: "kv-row", span { "Device" } span { "{preview.device_id}" } }
            div { class: "kv-row", span { "Revision read" } span { class: "num", "{preview.revision}" } }
            div { class: "kv-row", span { "Rows removed from device" } span { class: "num", "{thousands(preview.rows_removed)}" } }
            div { class: "kv-row", span { "Rows to send" } span { class: "num", "{thousands(preview.rows_added)}" } }
            div { class: "kv-row",
                span { "Resnapshot required" }
                span { if preview.resnapshot { "Yes" } else { "No" } }
            }
            div { class: "kv-row",
                span { "Required permission" }
                span { class: "status",
                    span { class: if granted { "status-dot ok" } else { "status-dot err" } }
                    span { class: "mono", "{preview.permission.required}" }
                    if granted {
                        " granted"
                    } else {
                        " not granted"
                    }
                }
            }
        }
        h4 { class: "inspector-section", "What changes" }
        ul { class: "cl-effects",
            for e in preview.effects.iter() {
                li { key: "{e}", "{e}" }
            }
        }
        for w in preview.warnings.iter() {
            div { key: "{w}", class: "callout warn",
                Ico { icon: Icon::TriangleAlert, size: 16 }
                span { "{w}" }
            }
        }
        if !granted {
            div { class: "callout err", role: "alert",
                Ico { icon: Icon::Lock, size: 16 }
                span { "This connection lacks {preview.permission.required}. Nothing can be applied here." }
            }
        }
        if let Some(e) = error {
            div { class: "field-error", role: "alert", "{e}" }
        }
        button {
            class: "btn primary block",
            disabled: busy || !preview.can_apply(),
            onclick: apply,
            "Apply change to {preview.name}"
        }
    }
}
