//! The leadership transfer review: what the transfer changes, what it needs,
//! and what to weigh. Nothing changes until the button in the footer.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::components::sheet::{Sheet, SheetFooter, SheetWidth};
use crate::services::backend::Backend;
use crate::state::cluster::{ClusterUi, TransferReview};
use crate::views::cluster::raft_tasks;

#[component]
pub fn TransferSheet(review: TransferReview) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let mut ui = use_context::<ClusterUi>().raft;
    let p = review.preview.clone();
    let mut close = move || ui.write().review = None;
    let (granted, required) = (p.permission.granted, p.permission.required.clone());
    rsx! {
        Sheet {
            title: format!("Review leadership transfer of {}", p.group),
            width: SheetWidth::Narrow,
            on_close: move |_| close(),
            footer: rsx! {
                SheetFooter {
                    error: review.error.clone(),
                    hint: Some("Applies to this group only. Other groups keep their leaders.".to_string()),
                    button { class: "btn", disabled: review.busy, onclick: move |_| close(), "Cancel" }
                    button {
                        class: "btn primary",
                        disabled: review.busy || !p.can_apply(),
                        onclick: move |_| {
                            let backend = backend.clone();
                            spawn_forever(async move { raft_tasks::apply_transfer(ui, backend).await });
                        },
                        "Transfer leadership to {p.to}"
                    }
                }
            },
            div { class: "review-feed",
                div { class: "review-block",
                    h3 { class: "section-title", "Transfer" }
                    div { class: "kv",
                        div { class: "kv-row", span { "Group" } span { class: "mono", "{p.group}" } }
                        div { class: "kv-row", span { "Group type" } span { "{p.kind.label()}" } }
                        div { class: "kv-row", span { "Current leader" } span { "{p.from}" } }
                        div { class: "kv-row", span { "New leader" } span { "{p.to}" } }
                        div { class: "kv-row", span { "Term" } span { class: "num", "{p.term} → {p.term + 1}" } }
                        div { class: "kv-row", span { "Target index lag" } span { class: "num", "{p.target_lag} entries" } }
                        div { class: "kv-row", span { "Voters reachable" } span { class: "num", "{p.voters_reachable} of {p.voters_total}" } }
                        div { class: "kv-row",
                            span { "Required permission" }
                            span { class: "status",
                                span { class: if granted { "status-dot ok" } else { "status-dot err" } }
                                span { class: "mono", "{required}" }
                                if granted { " granted" } else { " not granted" }
                            }
                        }
                    }
                }
                div { class: "review-block",
                    h3 { class: "section-title", "What changes" }
                    ul { class: "cl-effects",
                        for e in p.effects.iter() {
                            li { key: "{e}", "{e}" }
                        }
                    }
                }
                for w in p.warnings.iter() {
                    div { key: "{w}", class: "callout warn",
                        Ico { icon: Icon::TriangleAlert, size: 16 }
                        span { "{w}" }
                    }
                }
                if !granted {
                    div { class: "callout err", role: "alert",
                        Ico { icon: Icon::Lock, size: 16 }
                        span { "This connection lacks {required}. Ask an administrator for it. Nothing can be applied here." }
                    }
                }
            }
        }
    }
}
