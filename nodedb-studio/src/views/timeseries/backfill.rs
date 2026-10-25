//! Backfill: review one job that recomputes a rollup's buckets from the
//! retained source rows. A dry run reads and changes nothing. Review and Run
//! are separate steps, and a run starts only from a reviewed preview.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::AsyncView;
use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side, region_style};
use crate::models::ts_lifecycle::Lifecycle;
use crate::routes::Route;
use crate::routes::tabs::TimeseriesTab;
use crate::services::backend::Backend;
use crate::state::connection::ActiveConnection;
use crate::state::timeseries::{PreviewState, TimeseriesUi};
use crate::state::workspace::{Surface, Workspace};
use crate::views::timeseries::backfill_ops::buckets_title;
use crate::views::timeseries::backfill_panels::{
    BucketsBody, HistoryBody, JobForm, PreviewForm, RunBody,
};
use crate::views::timeseries::backfill_review::{ReviewSheet, RunDetails};
use crate::views::timeseries::backfill_tasks::{dry_run, resume};
use crate::views::timeseries::notice::TsNotice;

const SURFACE: Surface = Surface::Timeseries(TimeseriesTab::Backfill);
const LEFT_SIZE: PanelSize = PanelSize::width(560.0, 440.0, 860.0);
const PREVIEW_SIZE: PanelSize = PanelSize::height(350.0, 300.0, 560.0);
const JOB_SIZE: PanelSize = PanelSize::height(292.0, 260.0, 460.0);
const RUN_SIZE: PanelSize = PanelSize::height(245.0, 200.0, 340.0);

#[component]
pub fn BackfillTab() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let ts = use_context::<TimeseriesUi>();
    let mut ui_signal = ts.backfill;
    let workspace = use_context::<Signal<Workspace>>();
    let nav = use_navigator();

    let lifecycle_read = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let _scope = (
                conn(),
                active.read().as_ref().map(|c| c.current_database.clone()),
            );
            async move { backend.lifecycle("sales").await }
        })
    };
    let l: Option<Lifecycle> = lifecycle_read
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned();
    let load_error = lifecycle_read
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().err())
        .map(|e| e.to_string());

    let ui = ui_signal.read().clone();
    let target = ui.spec.target.clone();
    let history_read = {
        let backend = backend.clone();
        use_resource(use_reactive!(|target| {
            let backend = backend.clone();
            let _epoch = ui_signal.read().epoch;
            async move { backend.backfill_runs(Some(&target)).await }
        }))
    };
    let history = history_read.read().clone();

    // Arriving from Lifecycle starts the dry run for the form it handed over.
    use_effect({
        let backend = backend.clone();
        move || {
            let ready = lifecycle_read.read().as_ref().is_some_and(|r| r.is_ok());
            if !ready {
                return;
            }
            let go = {
                let u = ui_signal.peek();
                u.auto_preview && matches!(u.preview, PreviewState::Draft)
            };
            if go {
                ui_signal.write().auto_preview = false;
                spawn_forever(dry_run(ui_signal, backend.clone()));
            }
        }
    });

    let database = active
        .read()
        .as_ref()
        .map(|c| c.current_database.clone())
        .unwrap_or_default();
    let title = buckets_title(&ui.spec);
    let left_style = region_style(&workspace.read(), SURFACE, "columns", LEFT_SIZE);
    let detail = ui.detail.as_ref().and_then(|id| {
        history
            .as_ref()
            .and_then(|h| h.as_ref().ok())
            .and_then(|runs| runs.iter().find(|r| r.id == *id).cloned())
    });

    rsx! {
        div { class: "page-body ts-page",
            TsNotice { notice: ui.notice.clone(), on_dismiss: move |_| ui_signal.write().notice = None }
            div { class: "ts-backbar",
                button {
                    class: "link-btn",
                    onclick: move |_| { nav.push(Route::Timeseries { tab: TimeseriesTab::Lifecycle }); },
                    Ico { icon: Icon::ChevronLeft, size: 16 }
                    "Lifecycle"
                }
                span { class: "ts-muted", "Backfill of {ui.spec.target}. A dry run changes nothing." }
            }
            AsyncView {
                loading: l.is_none() && load_error.is_none(),
                empty: false,
                error: load_error,
            }
            if let Some(l) = l {
                div { class: "workbench events-wb ts-back",
                    div { class: "workbench col ts-back-left", style: "{left_style}",
                        Panel {
                            surface: SURFACE,
                            id: "preview",
                            title: "Backfill preview",
                            size: PREVIEW_SIZE,
                            PreviewForm { ui: ui.clone(), l: l.clone(), database: database.clone() }
                        }
                        ResizeHandle { surface: SURFACE, target: "preview", size: PREVIEW_SIZE, side: Side::Before }
                        Panel {
                            surface: SURFACE,
                            id: "buckets",
                            title,
                            class: "fill",
                            BucketsBody { ui: ui.clone() }
                        }
                    }
                    ResizeHandle { surface: SURFACE, target: "columns", size: LEFT_SIZE, side: Side::Before }
                    div { class: "workbench col ts-back-right",
                        Panel {
                            surface: SURFACE,
                            id: "job",
                            title: "Job identity",
                            size: JOB_SIZE,
                            JobForm { ui: ui.clone(), l: l.clone() }
                        }
                        ResizeHandle { surface: SURFACE, target: "job", size: JOB_SIZE, side: Side::Before }
                        Panel {
                            surface: SURFACE,
                            id: "run",
                            title: "Run backfill",
                            size: RUN_SIZE,
                            RunBody {
                                ui: ui.clone(),
                                on_dry_run: {
                                    let backend = backend.clone();
                                    move |_| { spawn_forever(dry_run(ui_signal, backend.clone())); }
                                },
                            }
                        }
                        ResizeHandle { surface: SURFACE, target: "run", size: RUN_SIZE, side: Side::Before }
                        Panel {
                            surface: SURFACE,
                            id: "history",
                            title: "Backfill history",
                            class: "fill",
                            match history {
                                Some(Ok(runs)) => rsx! {
                                    HistoryBody {
                                        runs,
                                        on_view: move |id: String| ui_signal.write().detail = Some(id),
                                        on_resume: {
                                            let backend = backend.clone();
                                            move |id: String| { spawn_forever(resume(ui_signal, backend.clone(), id)); }
                                        },
                                    }
                                },
                                Some(Err(e)) => rsx! { div { class: "callout err ts-panel-error", role: "alert", "{e}" } },
                                None => rsx! { div { class: "ts-state", "Loading…" } },
                            }
                        }
                    }
                    if let Some(kind) = ui.review {
                        ReviewSheet { ui: ui.clone(), kind }
                    }
                    if ui.detail.is_some() {
                        RunDetails { run: detail, on_close: move |_| ui_signal.write().detail = None }
                    }
                }
            }
        }
    }
}
