//! The Results panel: rows, the plan, and the messages of the last run.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::AsyncView;
use crate::components::icon::{Ico, Icon};
use crate::components::pager::Pager;
use crate::components::panel::Panel;
use crate::components::tabs::TabBar;
use crate::models::format::thousands;
use crate::models::workbench::{MessageLevel, ResultSet};
use crate::services::backend::Backend;
use crate::state::connection::ActiveConnection;
use crate::state::query::{QueryTab, QueryWorkspace, ResultsView, RunState, TxEpoch};
use crate::state::workspace::Surface;
use crate::views::query::export::ExportMenu;
use crate::views::query::plan::PlanView;
use crate::views::query::tasks::{self, Env};

const SURFACE: Surface = Surface::Query;

/// `7 rows · 142 ms`, beside the panel title.
fn summary(run: &RunState) -> Element {
    match run {
        RunState::Idle => rsx! {},
        RunState::Running => rsx! { span { class: "result-status", "Running\u{2026}" } },
        RunState::Failed(_) => rsx! {
            span { class: "result-status err",
                Ico { icon: Icon::CircleAlert, size: 16 }
                "Failed"
            }
        },
        RunState::Done(run) => {
            let r = &run.result;
            let rows = thousands(r.total_rows as u64);
            let unit = if r.total_rows == 1 { "row" } else { "rows" };
            rsx! {
                span { class: "result-status ok",
                    Ico { icon: Icon::CircleCheck, size: 16 }
                    "{rows} {unit} \u{b7} {r.elapsed_ms} ms"
                }
            }
        }
    }
}

#[component]
pub fn ResultsPanel() -> Element {
    let mut ws = use_context::<Signal<QueryWorkspace>>();
    let tab = ws.read().active().cloned();
    let page = tab.as_ref().and_then(|t| match &t.run {
        RunState::Done(run) => Some(run.result.clone()),
        RunState::Idle | RunState::Running | RunState::Failed(_) => None,
    });
    let extra = tab
        .as_ref()
        .map(|t| summary(&t.run))
        .unwrap_or_else(|| rsx! {});

    rsx! {
        Panel {
            surface: SURFACE,
            id: "results",
            title: "Results",
            class: "results-panel",
            title_extra: rsx! { {extra} },
            actions: rsx! { ExportMenu { result: page } },
            if let Some(tab) = tab {
                {
                    let view = tab.view;
                    let selected = ResultsView::ALL.iter().position(|v| *v == view).unwrap_or(0);
                    rsx! {
                        TabBar {
                            class: "sub results-tabs",
                            items: ResultsView::ALL.iter().map(|v| v.label()).collect::<Vec<_>>(),
                            active: selected,
                            on_select: move |i: usize| {
                                if let (Some(v), Some(t)) = (ResultsView::ALL.get(i), ws.write().active_mut()) {
                                    t.view = *v;
                                }
                            },
                        }
                        match view {
                            ResultsView::Results => rsx! { RowsView { tab: tab.clone() } },
                            ResultsView::Plan => rsx! { PlanView { tab: tab.clone() } },
                            ResultsView::Messages => rsx! { MessagesView { tab: tab.clone() } },
                        }
                    }
                }
            } else {
                div { class: "async-empty", "Open a query tab to see its results." }
            }
        }
    }
}

#[component]
fn RowsView(tab: QueryTab) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let ws = use_context::<Signal<QueryWorkspace>>();
    let tx = use_context::<TxEpoch>();
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let env = Env::new(ws, tx);
    match tab.run {
        RunState::Idle => rsx! {
            div { class: "async-empty", "Run the statement to see its rows. Press Ctrl+Enter in the editor." }
        },
        RunState::Running => {
            rsx! { AsyncView { loading: true, empty: false, error: None::<String> } }
        }
        RunState::Failed(msg) => rsx! {
            div { class: "result-error",
                div { class: "callout err", role: "alert",
                    Ico { icon: Icon::CircleAlert, size: 16 }
                    span { "{msg}" }
                }
            }
        },
        RunState::Done(run) => {
            let page = tab.page;
            rsx! {
                ResultTable {
                    result: run.result,
                    on_page: move |p: usize| {
                        let db = active.peek().as_ref().map(|c| c.current_database.clone()).unwrap_or_default();
                        tasks::start_run(env, backend.clone(), &db, p);
                    },
                    current: page,
                }
            }
        }
    }
}

/// The rows of one page, and the footer under them.
#[component]
pub fn ResultTable(result: ResultSet, current: usize, on_page: EventHandler<usize>) -> Element {
    let numeric: Vec<bool> = (0..result.columns().len())
        .map(|i| result.is_numeric(i))
        .collect();
    let pages = result.pages();
    rsx! {
        div { class: "results-wrap",
            div { class: "dt-wrap results-scroll",
                table { class: "dt results-table",
                    thead {
                        tr {
                            for (i, col) in result.columns().iter().enumerate() {
                                th { key: "{i}", class: if numeric[i] { "num" } else { "" }, "{col}" }
                            }
                        }
                    }
                    tbody {
                        for row in result.rows() {
                            tr { key: "{row.id}",
                                for (i, cell) in row.cells.iter().enumerate() {
                                    td { key: "{i}", class: if numeric[i] { "num" } else { "" }, "{cell}" }
                                }
                            }
                        }
                    }
                }
                if result.rows().is_empty() {
                    div { class: "async-empty", "The statement returned no rows." }
                }
            }
            div { class: "dt-footer results-footer",
                if pages > 1 {
                    if let Some((a, b)) = result.shown() {
                        span { "Showing {a}\u{2013}{b} of {result.total_rows} rows" }
                    }
                    span { class: "toolbar-spacer" }
                    Pager { page: current, pages, on_page }
                } else {
                    span { "Page 1 of 1" }
                    span { class: "toolbar-spacer" }
                    if !result.scanned.is_empty() || !result.access_path.is_empty() {
                        span { class: "scan-note", {scan_note(&result)} }
                    }
                }
            }
        }
    }
}

/// `Scanned 14M · index_scan(events_ts_idx)`.
fn scan_note(r: &ResultSet) -> String {
    let mut parts = Vec::new();
    if !r.scanned.is_empty() {
        parts.push(format!("Scanned {}", r.scanned));
    }
    if !r.access_path.is_empty() {
        parts.push(r.access_path.clone());
    }
    parts.join(" \u{b7} ")
}

#[component]
fn MessagesView(tab: QueryTab) -> Element {
    let rows: Vec<(MessageLevel, String)> = match &tab.run {
        RunState::Done(run) => run
            .messages
            .iter()
            .map(|m| (m.level, m.text.clone()))
            .collect(),
        RunState::Failed(msg) => vec![(MessageLevel::Warning, msg.clone())],
        RunState::Idle | RunState::Running => Vec::new(),
    };
    let failed = matches!(tab.run, RunState::Failed(_));
    if rows.is_empty() {
        return rsx! { div { class: "async-empty", "No messages yet." } };
    }
    rsx! {
        ul { class: "messages", "aria-label": "Messages",
            for (i, (level, text)) in rows.into_iter().enumerate() {
                li { key: "{i}", class: if failed { "message err" } else { "message" },
                    match (failed, level) {
                        (true, _) => rsx! { Ico { icon: Icon::CircleAlert, size: 16 } },
                        (false, MessageLevel::Info) => rsx! { Ico { icon: Icon::Info, size: 16 } },
                        (false, MessageLevel::Warning) => rsx! { Ico { icon: Icon::TriangleAlert, size: 16 } },
                    }
                    span { "{text}" }
                }
            }
        }
    }
}
