//! The Lifecycle review sheet: reads the pending changes in full, bound to
//! the catalog revision, and applies them.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::components::sheet::{Sheet, SheetFooter, SheetWidth};
use crate::models::ts_lifecycle::ChangeReview;
use crate::services::backend::Backend;
use crate::state::timeseries::TimeseriesUi;
use crate::views::events::common::SqlBlock;
use crate::views::timeseries::life_tasks;

#[component]
pub(super) fn ReviewSheet(
    review: Option<ChangeReview>,
    busy: bool,
    error: Option<String>,
    collection: String,
    pending: usize,
) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let ts = use_context::<TimeseriesUi>();
    let mut life = ts.life;
    let blocked = review.as_ref().is_some_and(|r| !r.blocked.is_empty());
    let nothing = pending == 0;
    let revision = review.as_ref().map(|r| r.revision);
    let footer_error = error
        .clone()
        .or_else(|| review.as_ref().and_then(|r| r.blocked.first().cloned()));

    rsx! {
        Sheet {
            title: "Review changes".to_string(),
            on_close: move |_| life.write().sheet = None,
            width: SheetWidth::Medium,
            footer: rsx! {
                SheetFooter {
                    error: footer_error,
                    hint: if nothing { Some("No change is pending.".to_string()) } else { Some("Applying records an operation you can audit.".to_string()) },
                    button { class: "btn", onclick: move |_| life.write().sheet = None, "Close" }
                    button {
                        class: "btn",
                        disabled: nothing,
                        title: "Remove every pending change without applying it",
                        onclick: move |_| {
                            let mut w = life.write();
                            w.pending.clear();
                            w.sheet = None;
                        },
                        "Discard all"
                    }
                    button {
                        class: "btn primary",
                        disabled: busy || blocked || nothing || revision.is_none(),
                        onclick: {
                            let (backend, collection) = (backend.clone(), collection.clone());
                            move |_| {
                                if let Some(rev) = revision {
                                    spawn_forever(life_tasks::apply(life, backend.clone(), collection.clone(), rev));
                                }
                            }
                        },
                        "Apply changes"
                    }
                }
            },
            div { class: "sheet-form ts-review",
                if busy && review.is_none() {
                    div { class: "ts-state", "Reading the catalog…" }
                }
                if nothing {
                    div { class: "empty-state",
                        Ico { icon: Icon::FileCode, size: 24 }
                        h2 { "No pending changes" }
                        p { "Edit the retention policy, add a rollup, or edit the late-data policy. Each is queued here, read in full, and applied only from this review." }
                    }
                }
                if let Some(r) = review {
                    p { class: "ts-muted", "Read at lifecycle revision {r.revision}. A newer revision invalidates this review." }
                    for (i , item) in r.items.iter().enumerate() {
                        div { key: "{i}", class: "ts-review-item",
                            h3 { "{item.title}" }
                            SqlBlock { sql: item.sql.clone(), class: "ts-sql" }
                            ul { class: "ts-impact",
                                for line in item.impact.iter() {
                                    li { key: "{line}", "{line}" }
                                }
                            }
                            if item.irreversible {
                                div { class: "callout warn",
                                    Ico { icon: Icon::TriangleAlert, size: 16 }
                                    span { "This change deletes data and cannot be undone." }
                                }
                            }
                        }
                    }
                    for b in r.blocked.iter() {
                        div { key: "{b}", class: "callout err", role: "alert",
                            Ico { icon: Icon::CircleAlert, size: 16 }
                            span { "{b}" }
                        }
                    }
                }
            }
        }
    }
}
