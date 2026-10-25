//! Explore: a collection over time. The builder shapes one query, Run
//! answers it, and the chart, the table, and the SQL all read that one
//! answer. Brushing the chart narrows the range and keeps the old one.

use std::rc::Rc;
use std::time::Duration;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::clipboard::copy_text;
use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::models::timeseries::ChartKind;
use crate::models::timeseries::{ExploreQuery, TimeCatalog, TimeSource, TimeSpan};
use crate::routes::Route;
use crate::routes::tabs::TimeseriesTab;
use crate::services::backend::Backend;
use crate::state::connection::ActiveConnection;
use crate::state::query::QueryWorkspace;
use crate::state::timeseries::{ExploreStatus, ExploreUi, Notice, TimeseriesUi};
use crate::state::workspace::Surface;
use crate::views::query::tasks::open_in_query;
use crate::views::timeseries::chart::TsChart;
use crate::views::timeseries::explore_query::QueryBody;
use crate::views::timeseries::explore_table::ResultsBody;
use crate::views::timeseries::explore_tasks::{init, run, set_range};
use crate::views::timeseries::notice::TsNotice;
use crate::views::timeseries::toolbar::ExploreToolbar;

const SURFACE: Surface = Surface::Timeseries(TimeseriesTab::Explore);
const RESULTS_SIZE: PanelSize = PanelSize::height(270.0, 130.0, 520.0);
const QUERY_SIZE: PanelSize = PanelSize::width(400.0, 300.0, 620.0);

/// Series the chart draws. The table lists them all.
const SERIES_LIMIT: usize = 5;

#[component]
pub fn ExploreTab() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let ui = use_context::<TimeseriesUi>();
    let mut ex = ui.explore;

    // The catalog belongs to the database in scope: a switch re-reads it.
    let mut catalog_read = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let _scope = (
                conn(),
                active.read().as_ref().map(|c| c.current_database.clone()),
            );
            async move { backend.time_sources().await }
        })
    };
    let state = resource_state(&catalog_read);
    let catalog: Option<TimeCatalog> = state.loaded().cloned();

    // Start from what the catalog offers, then answer the first query.
    use_effect({
        let backend = backend.clone();
        move || {
            let Some(Ok(c)) = catalog_read.read().clone() else {
                return;
            };
            init(&mut ex.write(), &c);
            let ready = {
                let e = ex.peek();
                e.draft.is_some() && e.shown.is_none() && e.seq == 0 && e.running.is_none()
            };
            if ready {
                spawn_forever(run(ex, backend.clone()));
            }
        }
    });

    // A refresh interval re-runs the query on a timer. The loop stops when
    // the tab goes, and never starts a run while one is waiting.
    use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            let every = ex.read().refresh.seconds();
            async move {
                let Some(secs) = every else { return };
                loop {
                    tokio::time::sleep(Duration::from_secs(secs)).await;
                    if ex.peek().running.is_none() {
                        spawn_forever(run(ex, backend.clone()));
                    }
                }
            }
        }
    });

    let e = ex.read().clone();
    let source: Option<TimeSource> = catalog.as_ref().and_then(|c| {
        e.draft
            .as_ref()
            .and_then(|q| c.source(&q.collection).cloned())
    });
    let issues = match (&e.draft, &source) {
        (Some(q), Some(s)) => q.check(s),
        _ => Vec::new(),
    };
    let nav = use_navigator();

    rsx! {
        div { class: "page-body ts-page",
            AsyncView {
                loading: state.is_loading(),
                empty: false,
                error: state.error_message(),
                retriable: state.is_retriable(),
                on_retry: move |_| catalog_read.restart(),
            }
            if state.is_empty() {
                div { class: "empty-state",
                    Ico { icon: Icon::ChartColumn, size: 24 }
                    h2 { "No collections" }
                    p { "This database holds no collection to explore over time. Refresh the catalog, or create one in the Designer." }
                    div { class: "actions",
                        button {
                            class: "btn",
                            onclick: move |_| catalog_read.restart(),
                            Ico { icon: Icon::RefreshCw, size: 16 }
                            "Refresh catalog"
                        }
                        button {
                            class: "btn",
                            onclick: move |_| { nav.push(Route::Designer {}); },
                            Ico { icon: Icon::Pencil, size: 16 }
                            "Create collection in Designer"
                        }
                    }
                }
            }
            if let (Some(c), Some(q), Some(s)) = (catalog.clone(), e.draft.clone(), source.clone()) {
                TsNotice { notice: e.notice.clone(), on_dismiss: move |_| ex.write().notice = None }
                ExploreToolbar {
                    catalog: c,
                    source: s.clone(),
                    query: q.clone(),
                    issues: issues.clone(),
                    running: e.running.is_some(),
                    on_run: {
                        let backend = backend.clone();
                        move |_| { spawn_forever(run(ex, backend.clone())); }
                    },
                    on_cancel: move |_| crate::views::timeseries::explore_tasks::cancel(ex),
                }
                ExploreBody { query: q, source: s, ui: e.clone() }
            }
        }
    }
}

/// The three panels under the builder.
#[component]
fn ExploreBody(query: ExploreQuery, source: TimeSource, ui: ExploreUi) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let tsui = use_context::<TimeseriesUi>();
    let mut ex = tsui.explore;
    let ws = use_context::<Signal<QueryWorkspace>>();
    let nav = use_navigator();
    let status = ui.status();
    let shown = ui.shown.clone();
    let changed = ui.changed();
    let title = shown
        .as_ref()
        .map(|r| r.query.title())
        .unwrap_or_else(|| query.title());
    let database = active
        .read()
        .as_ref()
        .map(|c| c.current_database.clone())
        .unwrap_or_default();

    let rerun = {
        let backend = backend.clone();
        move || {
            spawn_forever(run(ex, backend.clone()));
        }
    };
    let on_brush = {
        let rerun = rerun.clone();
        move |span: TimeSpan| {
            if set_range(&mut ex.write(), span) {
                rerun();
            }
        }
    };

    rsx! {
        div { class: "workbench ts-workbench",
            div { class: "workbench col ts-left",
                Panel {
                    surface: SURFACE,
                    id: "chart",
                    title,
                    class: "fill ts-chart-panel",
                    actions: rsx! { StatusText { status, changed } },
                    match (&shown, status) {
                        (None, ExploreStatus::Running) => rsx! { div { class: "ts-state", "Running…" } },
                        (None, _) => rsx! { div { class: "ts-state", "Press Run to draw the chart." } },
                        (Some(r), _) => rsx! {
                            if status == ExploreStatus::Error || status == ExploreStatus::Cancelled {
                                StaleBanner { status, error: ui.error.clone(), at: r.observed_at, on_retry: { let rerun = rerun.clone(); move |_| rerun() } }
                            }
                            TsChart {
                                result: r.clone(),
                                kind: ChartKind::Line,
                                hidden: ui.hidden.clone(),
                                limit: SERIES_LIMIT,
                                brushable: true,
                                stale: matches!(status, ExploreStatus::Error | ExploreStatus::Cancelled),
                                on_brush,
                                on_toggle: move |key: String| {
                                    let mut w = ex.write();
                                    if !w.hidden.remove(&key) {
                                        w.hidden.insert(key);
                                    }
                                },
                            }
                        },
                    }
                }
                ResizeHandle { surface: SURFACE, target: "results", size: RESULTS_SIZE, side: Side::After }
                Panel {
                    surface: SURFACE,
                    id: "results",
                    title: "Results",
                    size: RESULTS_SIZE,
                    actions: rsx! {
                        button {
                            class: "btn sm",
                            disabled: shown.is_none(),
                            title: "Copy every returned row as CSV. A filled point carries its provenance.",
                            onclick: {
                                let shown = shown.clone();
                                let hidden = ui.hidden.clone();
                                move |_| {
                                    let Some(r) = shown.clone() else { return };
                                    let csv = r.csv(&hidden);
                                    let rows = r.visible_rows(&hidden).len();
                                    spawn_forever(async move {
                                        let notice = match copy_text(csv).await {
                                            Ok(()) => Notice::ok(format!("Copied {rows} rows as CSV.")),
                                            Err(e) => Notice::err(e.to_string()),
                                        };
                                        ex.write().notice = Some(notice);
                                    });
                                }
                            },
                            Ico { icon: Icon::Copy, size: 16 }
                            "Copy CSV"
                        }
                    },
                    match shown.clone() {
                        Some(r) => rsx! {
                            ResultsBody {
                                result: r,
                                hidden: ui.hidden.clone(),
                                sort: ui.sort,
                                status,
                                on_sort: move |col| {
                                    let mut w = ex.write();
                                    w.sort = if w.sort.0 == col { (col, !w.sort.1) } else { (col, false) };
                                },
                            }
                        },
                        None => rsx! { div { class: "ts-state", "No result yet." } },
                    }
                }
            }
            ResizeHandle { surface: SURFACE, target: "query", size: QUERY_SIZE, side: Side::After }
            Panel {
                surface: SURFACE,
                id: "query",
                title: "Query",
                size: QUERY_SIZE,
                QueryBody {
                    draft: query.clone(),
                    source: source.clone(),
                    shown: shown.clone(),
                    tab: ui.tab,
                    changed,
                    on_tab: move |t| ex.write().tab = t,
                    on_gap: move |g| {
                        if let Some(q) = ex.write().draft.as_mut() {
                            q.gap = g;
                        }
                    },
                    on_edit_sql: {
                        let q = query.clone();
                        let backend = backend.clone();
                        move |_| {
                            let (backend, database) = (backend.clone(), database.clone());
                            let (title, sql) = (q.title(), q.sql());
                            spawn_forever(async move {
                                open_in_query(ws, backend, nav, database, title, sql).await;
                            });
                        }
                    },
                }
            }
        }
    }
}

/// The run's state beside the chart title.
#[component]
fn StatusText(status: ExploreStatus, changed: bool) -> Element {
    let (tone, text) = match (status, changed) {
        (ExploreStatus::Running, _) => ("info", "Running"),
        (ExploreStatus::Error, _) => ("err", "Last run failed"),
        (ExploreStatus::Cancelled, _) => ("idle", "Cancelled"),
        (_, true) => ("warn", "Draft changed. Not run"),
        (ExploreStatus::Empty, false) => ("idle", "No rows"),
        (ExploreStatus::Result, false) => ("ok", "Result"),
        (ExploreStatus::Draft, false) => ("idle", "Draft"),
    };
    rsx! {
        span { class: "status ts-status", role: "status",
            span { class: "status-dot {tone}" }
            "{text}"
        }
    }
}

/// A failed or cancelled run leaves the last result up, marked.
#[component]
fn StaleBanner(
    status: ExploreStatus,
    error: Option<String>,
    at: i64,
    on_retry: EventHandler<()>,
) -> Element {
    let when = crate::models::format::date_minute(at, 0);
    let text = match (status, error) {
        (ExploreStatus::Error, Some(e)) => {
            format!("The last run failed: {e} The chart shows the result from {when} UTC.")
        }
        _ => format!("Cancelled. The chart shows the last result, from {when} UTC."),
    };
    rsx! {
        div { class: "callout warn ts-stale", role: "alert",
            Ico { icon: Icon::TriangleAlert, size: 16 }
            span { class: "notice-text", "{text}" }
            button { class: "btn sm", onclick: move |_| on_retry.call(()), "Run again" }
        }
    }
}
