//! The Plan view: the operator tree, with EXPLAIN's estimates or ANALYZE's
//! measurements, and the details of the selected operator.
//!
//! EXPLAIN never shows a timing. Its plan has none, and the view says so
//! rather than leave a blank where a number would go.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::format::thousands;
use crate::models::query_plan::{ExplainMode, PlanOperator, QueryPlan, duration};
use crate::state::query::{PlanState, QueryTab, QueryWorkspace};

/// The second line of an operator card.
fn card_line(op: &PlanOperator, plan: &QueryPlan) -> String {
    match plan.mode {
        ExplainMode::Explain => match &op.target {
            Some(t) => t.clone(),
            None => format!("Est. rows: {}", thousands(op.est_rows)),
        },
        ExplainMode::Analyze => {
            let rows = op.actual_rows.map(thousands).unwrap_or_default();
            let time = op.time_us.map(duration).unwrap_or_default();
            format!(
                "Est. {} \u{b7} Actual {rows} \u{b7} {time}",
                thousands(op.est_rows)
            )
        }
    }
}

#[component]
pub fn PlanView(tab: QueryTab) -> Element {
    let mut ws = use_context::<Signal<QueryWorkspace>>();
    let mode = tab.mode;
    let shown = match &tab.plan {
        PlanState::Ready(plan) => Some(plan.mode),
        PlanState::None | PlanState::Running | PlanState::Failed(_) => None,
    };
    rsx! {
        div { class: "plan",
            div { class: "plan-bar",
                div { class: "choice-group", role: "radiogroup", "aria-label": "Plan mode",
                    for m in [ExplainMode::Explain, ExplainMode::Analyze] {
                        label {
                            key: "{m.label()}",
                            class: if mode == m { "choice selected" } else { "choice" },
                            input {
                                r#type: "radio",
                                class: "radio",
                                name: "plan-mode",
                                checked: mode == m,
                                onchange: move |_| {
                                    if let Some(t) = ws.write().active_mut() { t.mode = m; }
                                },
                            }
                            "{m.label()}"
                        }
                    }
                }
                span { class: "plan-note",
                    Ico { icon: Icon::Info, size: 16 }
                    "{mode.note()}"
                }
            }
            match &tab.plan {
                PlanState::None => rsx! {
                    div { class: "async-empty", "Press {mode.label()} in the toolbar to plan this statement." }
                },
                PlanState::Running => rsx! { div { class: "async-loading", "Planning\u{2026}" } },
                PlanState::Failed(msg) => rsx! {
                    div { class: "result-error",
                        div { class: "callout err", role: "alert",
                            Ico { icon: Icon::CircleAlert, size: 16 }
                            span { "{msg}" }
                        }
                    }
                },
                PlanState::Ready(plan) => rsx! {
                    if shown.is_some_and(|s| s != mode) {
                        div { class: "plan-stale", "Showing the {plan.mode.label()} plan. Press {mode.label()} to plan again in the other mode." }
                    }
                    PlanTree { plan: (**plan).clone(), selected: tab.selected_op }
                },
            }
        }
    }
}

#[component]
fn PlanTree(plan: QueryPlan, selected: Option<u32>) -> Element {
    let mut ws = use_context::<Signal<QueryWorkspace>>();
    let current = selected
        .and_then(|id| plan.operator(id))
        .or(plan.operators.first())
        .map(|o| o.id);
    let last = plan.operators.len().saturating_sub(1);
    rsx! {
        div { class: "plan-body",
            div { class: "plan-tree", role: "tree", "aria-label": "Operators",
                for (i, op) in plan.operators.iter().enumerate() {
                    {
                        let id = op.id;
                        let is_selected = current == Some(id);
                        rsx! {
                            button {
                                key: "{id}",
                                class: if is_selected { "op-card selected" } else { "op-card" },
                                role: "treeitem",
                                "aria-selected": "{is_selected}",
                                onclick: move |_| {
                                    if let Some(t) = ws.write().active_mut() { t.selected_op = Some(id); }
                                },
                                span { class: "op-icon", Ico { icon: op.kind.icon(), size: 20 } }
                                span { class: "op-text",
                                    span { class: "op-name", "{op.kind.label()}" }
                                    span { class: "op-line", {card_line(op, &plan)} }
                                }
                                span { class: "op-id", "#{id}" }
                            }
                            if i < last {
                                span { class: "op-link", "aria-hidden": "true", Ico { icon: Icon::ArrowUp, size: 16 } }
                            }
                        }
                    }
                }
            }
            if let Some(op) = current.and_then(|id| plan.operator(id)) {
                OperatorDetails { op: op.clone(), plan: plan.clone() }
            }
        }
    }
}

#[component]
fn OperatorDetails(op: PlanOperator, plan: QueryPlan) -> Element {
    let input = op.input.and_then(|id| plan.operator(id));
    rsx! {
        aside { class: "op-details", "aria-label": "Operator details",
            div { class: "inspector-head split",
                span { class: "op-head",
                    Ico { icon: op.kind.icon(), size: 22 }
                    h3 { class: "inspector-title", "{op.kind.label()}" }
                }
                span { class: "op-id", "#{op.id}" }
            }
            div { class: "kv kv-compact",
                div { class: "kv-row", span { "Operator type" } span { "{op.kind.label()}" } }
                div { class: "kv-row", span { "Estimated rows" } span { class: "num", {thousands(op.est_rows)} } }
                for (k, v) in op.facts.iter() {
                    div { key: "{k}", class: "kv-row", span { "{k}" } span { "{v}" } }
                }
                match (op.actual_rows, op.time_us) {
                    (Some(rows), Some(us)) => rsx! {
                        div { class: "kv-row", span { "Actual rows" } span { class: "num", {thousands(rows)} } }
                        div { class: "kv-row", span { "Time" } span { class: "num", {duration(us)} } }
                    },
                    _ => rsx! {
                        div { class: "kv-row", span { "Time" } span { class: "muted", "Not measured: estimates only" } }
                    },
                }
            }
            h4 { class: "inspector-section", "Description" }
            p { class: "op-description", "{op.description}" }
            h4 { class: "inspector-section", "Input" }
            match input {
                Some(i) => rsx! {
                    div { class: "op-input",
                        span { class: "op-id", "#{i.id}" }
                        span { class: "op-text",
                            span { class: "op-name", "{i.kind.label()}" }
                            span { class: "op-line", {card_line(i, &plan)} }
                        }
                    }
                },
                None => rsx! { p { class: "muted", "Reads from storage." } },
            }
        }
    }
}
