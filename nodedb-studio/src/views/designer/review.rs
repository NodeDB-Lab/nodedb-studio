//! The Review changes panel: staged changes, the DDL they generate, what a
//! review can and cannot report, the dry run, and Apply.
//!
//! Apply is the only way a staged change reaches the database, and it is
//! offered here after the review. The panel never states a lock or a scan
//! cost the server did not report, and never promises one transaction.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::AsyncView;
use crate::components::clipboard::use_copier;
use crate::components::icon::{Ico, Icon};
use crate::models::designer_change::{ChangeReview, DryRun, PendingChange};
use crate::services::async_state::AsyncState;
use crate::services::backend::Backend;
use crate::state::designer::{ApplyResult, DesignerState, ReviewState, ReviewTab};
use crate::views::designer::ddl_view::Ddl;
use crate::views::designer::tasks;

const NOT_REPORTED: &str = "Not reported";

fn jump(anchor: &str) {
    let _ = document::eval(&format!(
        "document.getElementById('{anchor}')?.scrollIntoView({{ block: 'nearest' }});"
    ));
}

#[component]
pub fn ReviewPanel(
    pending: Vec<PendingChange>,
    review: AsyncState<ChangeReview>,
    state_of: ReviewState,
    on_retry: EventHandler<()>,
) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let mut state = use_context::<Signal<DesignerState>>();
    let copier = use_copier();
    let loaded = review.loaded().cloned();
    let n = pending.len();
    let can_apply = loaded.as_ref().is_some_and(ChangeReview::can_apply) && n > 0 && !state_of.busy;
    let apply_title = match (&loaded, n) {
        (_, 0) => "No staged changes".to_string(),
        (Some(r), _) => r
            .blocked
            .clone()
            .or_else(|| {
                (!r.unsupported.is_empty())
                    .then(|| "A staged change cannot be generated".to_string())
            })
            .or_else(|| {
                matches!(r.dry_run, DryRun::Failed { .. }).then(|| "The dry run failed".to_string())
            })
            .unwrap_or_else(|| "Apply the staged changes to the database".to_string()),
        (None, _) => "Waiting for the review".to_string(),
    };
    let apply = {
        let backend = backend.clone();
        move |_| {
            let backend = backend.clone();
            spawn_forever(async move { tasks::apply(state, backend).await });
        }
    };
    rsx! {
        aside { class: "sheet review-panel", "aria-label": "Review changes",
            header { class: "panel-header",
                span { class: "panel-grip", "aria-hidden": "true",
                    Ico { icon: Icon::GripVertical, size: 16 }
                }
                h2 { class: "panel-title", "Review changes" }
                div { class: "panel-actions" }
                button {
                    class: "icon-btn panel-close",
                    "aria-label": "Close Review changes",
                    onclick: move |_| state.write().review = None,
                    Ico { icon: Icon::X, size: 16 }
                }
            }
            div { class: "sheet-body review-body",
                div { class: "review-summary",
                    span { class: "review-count", "{n}" }
                    div {
                        div { class: "review-headline",
                            if n == 1 { "1 pending change" } else { "{n} pending changes" }
                        }
                        div { class: "insp-sub", "Review the changes below before applying them." }
                    }
                    button {
                        class: "link-btn review-discard",
                        disabled: n == 0,
                        onclick: {
                            let backend = backend.clone();
                            move |_| {
                                let backend = backend.clone();
                                spawn_forever(async move { tasks::discard_all(state, backend).await });
                            }
                        },
                        "Discard all"
                    }
                }
                div { class: "review-list",
                    for (k , change) in pending.iter().enumerate() {
                        {
                            let id = change.id.clone();
                            let open = state_of.open.contains(&change.id);
                            let block = loaded
                                .as_ref()
                                .and_then(|r| r.blocks.iter().find(|b| b.change_id == change.id))
                                .cloned();
                            let metadata = !change.runs_ddl();
                            rsx! {
                                div { key: "{change.id}", class: "review-item",
                                    button {
                                        class: "review-item-head",
                                        "aria-expanded": "{open}",
                                        onclick: {
                                            let id = id.clone();
                                            move |_| {
                                                if let Some(r) = state.write().review.as_mut()
                                                    && !r.open.remove(&id)
                                                {
                                                    r.open.insert(id.clone());
                                                }
                                            }
                                        },
                                        span { class: "step-num", "{k + 1}" }
                                        span { class: "review-item-text",
                                            span { class: "review-item-title", "{change.title()}" }
                                            span { class: "review-item-detail",
                                                "{change.detail()}"
                                                if metadata {
                                                    " · workspace metadata"
                                                }
                                            }
                                        }
                                        Ico { icon: if open { Icon::ChevronUp } else { Icon::ChevronDown }, size: 16 }
                                    }
                                    if open {
                                        div { class: "review-item-body",
                                            if let Some(b) = block {
                                                Ddl { lines: b.lines, diff: true }
                                            }
                                            button {
                                                class: "link-btn",
                                                onclick: {
                                                    let (backend, id) = (backend.clone(), id.clone());
                                                    move |_| {
                                                        let (backend, id) = (backend.clone(), id.clone());
                                                        spawn_forever(async move { tasks::discard(state, backend, id).await });
                                                    }
                                                },
                                                Ico { icon: Icon::Trash, size: 16 }
                                                "Remove from draft"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                div { class: "tab-bar sub review-tabs", role: "tablist",
                    for tab in ReviewTab::ALL {
                        button {
                            key: "{tab.label()}",
                            class: if tab == state_of.tab { "tab active" } else { "tab" },
                            role: "tab",
                            "aria-selected": "{tab == state_of.tab}",
                            onclick: move |_| {
                                if let Some(r) = state.write().review.as_mut() {
                                    r.tab = tab;
                                }
                                jump(tab.anchor());
                            },
                            "{tab.label()}"
                        }
                    }
                }
                AsyncView {
                    loading: review.is_loading(),
                    empty: false,
                    error: review.error_message(),
                    retriable: review.is_retriable(),
                    on_retry: move |_| on_retry.call(()),
                }
                if let Some(r) = loaded.clone() {
                    section { id: "review-ddl", class: "review-section",
                        div { class: "ddl-wrap",
                            Ddl {
                                lines: r.script().lines().map(str::to_string).collect::<Vec<_>>(),
                                diff: true,
                            }
                            button {
                                class: "icon-btn sm ddl-copy",
                                "aria-label": "Copy DDL",
                                title: "Copy DDL",
                                onclick: {
                                    let script = r.script();
                                    move |_| copier.copy(script.clone())
                                },
                                Ico { icon: Icon::Copy, size: 16 }
                            }
                        }
                        for u in r.unsupported.iter() {
                            div { key: "{u.change_id}", class: "callout err",
                                Ico { icon: Icon::CircleAlert, size: 16 }
                                span { "{u.reason} Nothing is generated for it." }
                            }
                        }
                    }
                    section { id: "review-impact", class: "review-section",
                        h3 { "Dependent objects check" }
                        if r.dependents.is_empty() {
                            div { class: "review-check ok",
                                Ico { icon: Icon::CircleCheck, size: 16 }
                                "No dependent objects will be affected."
                            }
                        } else {
                            div { class: "review-check warn",
                                Ico { icon: Icon::TriangleAlert, size: 16 }
                                "{r.dependents.len()} dependent object(s) are affected."
                            }
                            ul { class: "review-bullets",
                                for d in r.dependents.iter() {
                                    li { key: "{d}", "{d}" }
                                }
                            }
                        }
                        div { class: "kv review-kv",
                            div { class: "kv-row",
                                span { "Data scan / backfill impact" }
                                span { class: "muted", "{r.scan.clone().unwrap_or_else(|| NOT_REPORTED.to_string())}" }
                            }
                            div { class: "kv-row",
                                span { "Lock behavior" }
                                span { class: "muted", "{r.locks.clone().unwrap_or_else(|| NOT_REPORTED.to_string())}" }
                            }
                        }
                    }
                    section { id: "review-dry-run", class: "review-section",
                        h3 { "Dry run" }
                        match r.dry_run.clone() {
                            DryRun::Completed { warnings } if warnings.is_empty() => rsx! {
                                div { class: "review-check ok",
                                    Ico { icon: Icon::CircleCheck, size: 16 }
                                    "Completed with no warnings"
                                }
                            },
                            DryRun::Completed { warnings } => rsx! {
                                div { class: "review-check warn",
                                    Ico { icon: Icon::TriangleAlert, size: 16 }
                                    "Completed with {warnings.len()} warning(s)"
                                }
                                ul { class: "review-bullets",
                                    for w in warnings.iter() {
                                        li { key: "{w}", "{w}" }
                                    }
                                }
                            },
                            DryRun::Failed { errors } => rsx! {
                                div { class: "review-check err",
                                    Ico { icon: Icon::CircleAlert, size: 16 }
                                    "Failed. Nothing was applied."
                                }
                                ul { class: "review-bullets",
                                    for e in errors.iter() {
                                        li { key: "{e}", "{e}" }
                                    }
                                }
                            },
                        }
                        p { class: "insp-sub",
                            "Statements run one after another, not as one transaction. If one fails, the ones before it stay applied."
                        }
                    }
                    if let Some(why) = r.blocked.clone() {
                        div { class: "callout warn",
                            Ico { icon: Icon::TriangleAlert, size: 16 }
                            span { "{why}" }
                        }
                    }
                }
                if let Some(result) = state_of.result.clone() {
                    match result {
                        ApplyResult::Applied(_) => rsx! {},
                        ApplyResult::Partial { applied, failed, reason } => rsx! {
                            div { class: "callout err", role: "alert",
                                Ico { icon: Icon::CircleAlert, size: 16 }
                                span {
                                    "{applied} applied, then {failed} failed: {reason} The earlier changes stay applied. The rest stay staged."
                                }
                            }
                        },
                        ApplyResult::Failed(reason) => rsx! {
                            div { class: "callout err", role: "alert",
                                Ico { icon: Icon::CircleAlert, size: 16 }
                                span { "Nothing was applied: {reason}" }
                            }
                        },
                    }
                }
            }
            footer { id: "review-permissions", class: "sheet-footer review-footer",
                div { class: "review-identity",
                    div { class: "kv-line",
                        span { "Your permission" }
                        strong { "{loaded.as_ref().map(|r| r.permission.clone()).unwrap_or_default()}" }
                    }
                    div { class: "kv-line",
                        span { "Target" }
                        strong { "{loaded.as_ref().map(|r| r.target.clone()).unwrap_or_default()}" }
                    }
                }
                div { class: "toolbar-spacer" }
                button { class: "btn", onclick: move |_| state.write().review = None, "Cancel" }
                button {
                    class: "btn primary",
                    disabled: !can_apply,
                    title: "{apply_title}",
                    onclick: apply,
                    if state_of.busy { "Applying…" } else { "Apply changes" }
                }
            }
        }
    }
}
