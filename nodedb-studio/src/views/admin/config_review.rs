//! The Proposal review panel: what the proposed values would do on the
//! chosen target. It reads the proposal from the server each time the
//! proposals change, and applies it against the revision it read.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::models::admin::AdminChange;
use crate::models::admin_config::{ConfigRequest, ConfigScope, ConfigTarget, Validation};
use crate::models::feeds::Tone;
use crate::services::backend::Backend;
use crate::state::admin::{AdminUi, ConfigUi};
use crate::state::events::feeds::Notice;
use crate::views::events::common::StatusText;

async fn apply(
    mut ui: Signal<ConfigUi>,
    backend: Rc<dyn Backend>,
    req: ConfigRequest,
    revision: u64,
) {
    {
        let mut w = ui.write();
        w.busy = true;
        w.error = None;
    }
    let result = backend
        .apply_admin_change(&AdminChange::Config(req.clone()), revision)
        .await;
    let mut w = ui.write();
    w.busy = false;
    match result {
        Ok(done) => {
            w.notice = Some(Notice::ok(format!(
                "{} Operation {}.",
                done.summary, done.operation
            )));
            w.clear(&req.target);
            w.epoch += 1;
        }
        Err(e) => w.error = Some(e.to_string()),
    }
}

#[component]
pub fn ConfigReview(target: ConfigTarget, selected: Option<String>) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut ui = use_context::<AdminUi>().config;
    let epoch = use_memo(move || ui.read().epoch);
    let request = {
        let target = target.clone();
        use_memo(move || ConfigRequest {
            target: target.clone(),
            changes: ui.read().changes_for(&target),
        })
    };
    let mut proposal = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            let _conn = conn();
            let _epoch = epoch();
            let req = request();
            async move {
                if req.changes.is_empty() {
                    return Ok(None);
                }
                backend.preview_config(&req).await.map(Some)
            }
        }
    });
    let state = resource_state(&proposal);
    let req = request();
    let busy = ui.read().busy;
    let error = ui.read().error.clone();
    let Some(p) = state.loaded().cloned().flatten() else {
        return rsx! {
            AsyncView {
                loading: state.is_loading(),
                empty: false,
                error: state.error_message(),
                retriable: state.is_retriable(),
                on_retry: move |_| proposal.restart(),
            }
            if !state.is_loading() && state.error_message().is_none() {
                div { class: "ad-review-empty",
                    p { "No change is proposed for this target." }
                    p { class: "muted", "Pick a proposed value in the table to review it here." }
                }
            }
        };
    };
    let Some(item) = p.item_for(selected.as_deref()).cloned() else {
        return rsx! {};
    };
    let count = p.items.len();
    let scope_word = match target.scope {
        ConfigScope::Cluster => "cluster",
        ConfigScope::Node => "node",
    };
    let (granted, required) = (p.permission.granted, p.permission.required.clone());
    let cancel_target = target.clone();
    let revision = p.revision;
    rsx! {
        div { class: "inspector ad-proposal",
            if count > 1 {
                div { class: "cl-chips ad-chips",
                    for i in p.items.iter() {
                        button {
                            key: "{i.key}",
                            class: if i.key == item.key { "chip active" } else { "chip" },
                            onclick: {
                                let k = i.key.clone();
                                move |_| ui.write().selected = Some(k.clone())
                            },
                            "{i.key}"
                        }
                    }
                }
            }
            div { class: "kv",
                div { class: "kv-row", span { "Target" } span { class: "mono", "{p.target_name}" } }
                div { class: "kv-row", span { "Setting" } span { class: "mono", "{item.key}" } }
                div { class: "kv-row", span { "Effective value (proposed)" } strong { "{item.effective}" } }
                div { class: "kv-row", span { "Current value" } span { "{item.current}" } }
                div { class: "kv-row", span { "Inherited cluster default" } span { "{item.inherited}" } }
            }
            div { class: "kv",
                div { class: "kv-row", span { "Difference" } span { class: "num", "{item.difference}" } }
                div { class: "kv-row", span { "Source" } span { "{item.source_after}" } }
                div { class: "kv-row ad-tall",
                    span { "Permission check" }
                    div { class: "ad-stack",
                        StatusText {
                            tone: if granted { Tone::Ok } else { Tone::Err },
                            text: if granted { "Allowed".to_string() } else { "Not allowed".to_string() },
                        }
                        span { class: "hint",
                            if granted {
                                "You have permission to modify {scope_word} config."
                            } else {
                                "This connection lacks {required}."
                            }
                        }
                    }
                }
                div { class: "kv-row ad-tall",
                    span { "Restart impact" }
                    div { class: "ad-stack",
                        if item.restart_required {
                            span { "Restart {p.target_name}" }
                            span { class: "hint", "The value takes effect after the restart." }
                        } else {
                            span { "No restart required" }
                            span { class: "hint", "Change takes effect at runtime." }
                        }
                    }
                }
                div { class: "kv-row ad-tall",
                    span { "Validation" }
                    div { class: "ad-stack",
                        match item.validation.clone() {
                            Validation::Passed => rsx! {
                                StatusText { tone: Tone::Ok, text: "Passed".to_string() }
                                span { class: "hint", "Value is valid ({item.range})." }
                            },
                            Validation::Failed(why) => rsx! {
                                StatusText { tone: Tone::Err, text: "Failed".to_string() }
                                span { class: "hint", "{why}" }
                            },
                        }
                    }
                }
            }
            div { class: "callout info",
                Ico { icon: Icon::Info, size: 16 }
                span {
                    "Changes are proposed for {scope_word} {p.target_name}. They are not applied until you confirm."
                    if count > 1 {
                        " {count} settings are proposed."
                    }
                }
            }
            if let Some(e) = error {
                div { class: "callout err", role: "alert",
                    Ico { icon: Icon::CircleAlert, size: 16 }
                    span { "{e}" }
                }
            }
            div { class: "inspector-actions",
                button {
                    class: "btn",
                    disabled: busy,
                    onclick: move |_| ui.write().clear(&cancel_target),
                    "Cancel"
                }
                button {
                    class: "btn primary",
                    disabled: busy || !p.can_apply(),
                    onclick: move |_| {
                        let (backend, req) = (backend.clone(), req.clone());
                        spawn_forever(async move { apply(ui, backend, req, revision).await });
                    },
                    Ico { icon: Icon::Play, size: 16 }
                    "Apply reviewed changes"
                }
            }
        }
    }
}
