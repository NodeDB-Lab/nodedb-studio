//! The rebalance plan review: each move, the group it changes, what it needs,
//! and what to weigh. Nothing moves until the button in the footer.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::components::sheet::{Sheet, SheetFooter, SheetWidth};
use crate::models::cluster_place::{Placement, size_text};
use crate::services::backend::Backend;
use crate::state::cluster::{ClusterUi, PlanReview};
use crate::views::cluster::place_tasks;
use crate::views::events::common::StatusText;

#[component]
pub fn PlanSheet(review: PlanReview, placement: Placement) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let mut ui = use_context::<ClusterUi>().place;
    let plan = review.plan.clone();
    let mut close = move || ui.write().review = None;
    let n = plan.moves.len();
    let (granted, required) = (plan.permission.granted, plan.permission.required.clone());
    let label = if n == 1 {
        "Start 1 replica move".to_string()
    } else {
        format!("Start {n} replica moves")
    };
    rsx! {
        Sheet {
            title: "Review rebalance plan".to_string(),
            width: SheetWidth::Medium,
            on_close: move |_| close(),
            footer: rsx! {
                SheetFooter {
                    error: review.error.clone(),
                    hint: Some("Starting the plan creates one operation with a stable id.".to_string()),
                    button { class: "btn", disabled: review.busy, onclick: move |_| close(), "Back to plan" }
                    button {
                        class: "btn primary",
                        disabled: review.busy || !plan.can_apply(),
                        onclick: move |_| {
                            let backend = backend.clone();
                            spawn_forever(async move { place_tasks::apply(ui, backend).await });
                        },
                        "{label}"
                    }
                }
            },
            div { class: "review-feed",
                div { class: "review-block",
                    h3 { class: "section-title", "Replica moves" }
                    div { class: "dt-wrap",
                        table { class: "dt cl-table",
                            thead {
                                tr {
                                    th { "Group" }
                                    th { "Replica" }
                                    th { "Source" }
                                    th { "Destination" }
                                    th { class: "num", "Estimated bytes" }
                                    th { "Estimated impact" }
                                }
                            }
                            tbody {
                                for m in plan.moves.iter() {
                                    tr { key: "{m.group}",
                                        td { class: "key", "{m.group}" }
                                        td { "{m.replica}" }
                                        td { "{m.source}" }
                                        td { "{m.destination}" }
                                        td { class: "num", "{size_text(m.estimated_bytes)}" }
                                        td { StatusText { tone: m.impact.tone(), text: m.impact.label().to_string() } }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "meta-line", "Total to copy: {size_text(plan.total_bytes())}. The server does not estimate query impact." }
                }
                div { class: "review-block",
                    h3 { class: "section-title", "Affected groups" }
                    div { class: "kv",
                        for m in plan.moves.iter() {
                            {
                                let q = placement.quorum(&m.group);
                                let shards = m.vshards.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", ");
                                rsx! {
                                    div { key: "{m.group}", class: "kv-row",
                                        span { class: "mono", "{m.group}" }
                                        span {
                                            "vShards {shards}"
                                            if let Some(q) = q {
                                                " · {q.reachable} of {q.voters} voters reachable, quorum {q.quorum}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
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
                div { class: "callout info",
                    Ico { icon: Icon::Info, size: 16 }
                    span { "A replica move changes the membership of its Raft group, so it reaches every vShard of that group. The destination joins as a learner and catches up before it votes." }
                }
                for w in plan.warnings.iter() {
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
