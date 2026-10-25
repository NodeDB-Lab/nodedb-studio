//! The Build history panel: the builds of one index, each with a stable id
//! and the state it ended in. Progress never shows as a percentage: a build
//! has no measured denominator.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::format::date_second;
use crate::models::search_index::{RunHistory, RunState};
use crate::state::search::SearchUi;
use crate::views::search::indexes::edit;

/// How many runs the history shows before View all runs.
const LATEST: usize = 3;

pub fn run_dot(s: &RunState) -> &'static str {
    match s {
        RunState::Completed => "status-dot ok",
        RunState::Queued | RunState::Running => "status-dot info",
        RunState::Cancelled => "status-dot idle",
        RunState::Error { .. } => "status-dot err",
    }
}

#[component]
pub fn HistoryBody(
    names: Vec<String>,
    index: Option<String>,
    history: Option<RunHistory>,
    loading: bool,
    error: Option<String>,
    all_runs: bool,
) -> Element {
    let ui = use_context::<SearchUi>();
    let i = ui.indexes;
    let runs = history.map(|h| h.runs).unwrap_or_default();
    let total = runs.len();
    let shown: Vec<_> = if all_runs {
        runs
    } else {
        runs.into_iter().take(LATEST).collect()
    };
    rsx! {
        div { class: "toolbar nowrap srch-ix-bar",
            label { r#for: "srch-hist-index", class: "toolbar-label", "Index" }
            select {
                id: "srch-hist-index",
                class: "select native",
                disabled: names.is_empty(),
                onchange: move |e| {
                    let v = e.value();
                    edit(i, |s| {
                        s.history_index = Some(v);
                        s.all_runs = false;
                    });
                },
                for n in names.iter() {
                    option { key: "{n}", value: "{n}", selected: index.as_deref() == Some(n.as_str()), "{n}" }
                }
            }
        }
        if let Some(message) = error {
            div { class: "callout err srch-note", role: "alert",
                Ico { icon: Icon::CircleAlert, size: 16 }
                span { "{message}" }
            }
        }
        if loading && shown.is_empty() {
            div { class: "srch-state", role: "status", "Reading the builds…" }
        } else if shown.is_empty() && index.is_some() {
            div { class: "srch-state", "No build has run for this index." }
        } else if !shown.is_empty() {
            div { class: "dt-wrap srch-hist-table",
                table { class: "dt", "aria-label": "Build history",
                    thead {
                        tr { th { "Run" } th { "Requested (UTC)" } th { "State" } th { "Action" } }
                    }
                    tbody {
                        for r in shown.iter() {
                            {
                                let id = r.id.clone();
                                rsx! {
                                    tr { key: "{r.id}",
                                        td { class: "mono", "{r.id}" }
                                        td { class: "num-left", {date_second(r.requested_at, 0)} }
                                        td {
                                            span { class: "status",
                                                span { class: "{run_dot(&r.state)}" }
                                                "{r.state.text()}"
                                            }
                                        }
                                        td {
                                            button {
                                                class: "link-btn",
                                                "aria-label": "View run {r.id}",
                                                onclick: move |_| edit(i, |s| s.run = Some(id.clone())),
                                                "View run"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if total > LATEST {
                div { class: "srch-hist-foot",
                    button {
                        class: "link-btn",
                        "aria-expanded": "{all_runs}",
                        onclick: move |_| edit(i, |s| s.all_runs = !s.all_runs),
                        Ico { icon: Icon::List, size: 16 }
                        if all_runs { "Show the latest {LATEST}" } else { "View all runs" }
                    }
                    if all_runs {
                        span { class: "srch-foot-note", "{total} runs" }
                    }
                }
            }
        }
    }
}
