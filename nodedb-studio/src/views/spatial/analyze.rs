//! The Analyze tab: a typed spatial operation, its rows, and its SQL.
//!
//! Run asks the server. Changing the form marks the shown rows stale and
//! never replaces them. The SQL is written from the checked form, so the two
//! always agree.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::models::format::thousands;
use crate::models::geo_ops::Bbox;
use crate::models::spatial::{GeoSource, SpatialSetup};
use crate::models::spatial_analyze::{AnalysisForm, Availability, Issue};
use crate::routes::Route;
use crate::routes::tabs::SpatialTab;
use crate::services::backend::Backend;
use crate::state::spatial::{AnalyzeDraft, ResultLayer, SpatialUi};
use crate::state::workspace::Surface;
use crate::views::spatial::analyze_form::ComposerBody;
use crate::views::spatial::analyze_query::QueryBody;
use crate::views::spatial::analyze_results::ResultsBody;

const SURFACE: Surface = Surface::Spatial(SpatialTab::Analyze);
const ANALYSIS_SIZE: PanelSize = PanelSize::height(262.0, 170.0, 420.0);
const QUERY_SIZE: PanelSize = PanelSize::width(380.0, 300.0, 580.0);

/// The availability of the form's operation on this connection.
pub fn availability(setup: &SpatialSetup, form: &AnalysisForm) -> Availability {
    setup
        .operations
        .iter()
        .find(|o| o.op == form.op)
        .map_or(Availability::Supported, |o| o.availability.clone())
}

#[component]
pub fn AnalyzeTab(setup: SpatialSetup, source: GeoSource) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let ui = use_context::<SpatialUi>();
    let nav = use_navigator();
    let mut analyze = ui.analyze;
    let mut map_ui = ui.map;
    let key = source.source.key();

    // Typing in the form changes the draft, not this memo, so only a new run
    // restarts the read.
    let pending = use_memo(use_reactive!(|key| {
        analyze
            .read()
            .drafts
            .get(&key)
            .and_then(|d| d.pending.clone())
    }));
    let run = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let _conn = conn();
            let pending = pending();
            async move {
                match pending {
                    Some((seq, req)) => Some((seq, backend.spatial_analyze(&req).await)),
                    None => None,
                }
            }
        })
    };
    // A finished run lands in the draft. A late answer to a run that was
    // canceled or replaced no longer matches `pending` and is dropped.
    use_effect(use_reactive!(|key| {
        let Some(Some((seq, outcome))) = run.read().clone() else {
            return;
        };
        let mut a = analyze.write();
        let d = a.draft_mut(&key);
        if d.pending.as_ref().map(|p| p.0) != Some(seq) {
            return;
        }
        d.pending = None;
        match outcome {
            Ok(r) => {
                d.selected = r.rows.first().map(|row| row.id.clone());
                d.shown = Some(r);
                d.error = None;
            }
            // The last rows stay, labeled as the last run's.
            Err(e) => d.error = Some(e.to_string()),
        }
    }));

    let d: AnalyzeDraft = analyze.read().draft(&key);
    let avail = availability(&setup, &d.form);
    let checked = d.form.check(&source, &avail);
    let issues: Vec<Issue> = checked.as_ref().err().cloned().unwrap_or_default();
    let request = checked.as_ref().ok().cloned();
    let stale = match (&d.shown, &request) {
        (Some(shown), Some(req)) => shown.request != *req,
        (Some(_), None) => true,
        (None, _) => false,
    };
    let running = d.pending.is_some();
    let sql = request.as_ref().map(|r| r.sql());
    let shown = d.shown.clone();
    let count = shown.as_ref().map(|r| (r.rows.len(), r.matching));

    let run_now = {
        let (key, request) = (key.clone(), request.clone());
        move |_| {
            let mut a = analyze.write();
            let draft = a.draft_mut(&key);
            draft.tried = true;
            if let Some(req) = request.clone() {
                draft.seq += 1;
                draft.pending = Some((draft.seq, req));
                draft.error = None;
            }
        }
    };
    let cancel = {
        let key = key.clone();
        move |_| analyze.write().draft_mut(&key).pending = None
    };
    let show_on_map = {
        let shown = shown.clone();
        move |_| {
            let Some(r) = shown.clone() else { return };
            let query = r.request.query_shape();
            let layer = ResultLayer {
                title: r.request.title(),
                query: query.clone(),
                rows: r.rows.iter().map(|row| (row.id.clone(), row.at)).collect(),
                count: r.rows.len(),
            };
            {
                let mut m = map_ui.write();
                m.result = Some(layer);
                m.hidden.remove("result");
                m.frame = query.bbox().map(|b: Bbox| b.grown(0.12));
            }
            nav.push(Route::Spatial {
                tab: SpatialTab::Map,
            });
        }
    };

    rsx! {
        div { class: "workbench sp-analyze-wb",
            div { class: "workbench col sp-an-left",
                Panel {
                    surface: SURFACE,
                    id: "analysis",
                    title: "Analysis",
                    size: ANALYSIS_SIZE,
                    ComposerBody {
                        setup: setup.clone(),
                        source: source.clone(),
                        draft: d.clone(),
                        issues: issues.clone(),
                        running,
                        on_run: run_now,
                        on_cancel: cancel,
                    }
                }
                ResizeHandle { surface: SURFACE, target: "analysis", size: ANALYSIS_SIZE, side: Side::Before }
                Panel {
                    surface: SURFACE,
                    id: "results",
                    title: "Results",
                    class: "fill",
                    title_extra: rsx! {
                        if let Some((rows, matching)) = count {
                            span { class: "sp-count-note",
                                if rows as u64 == matching {
                                    if rows == 1 { "1 result" } else { "{thousands(rows as u64)} results" }
                                } else {
                                    "{thousands(rows as u64)} of {thousands(matching)} results"
                                }
                            }
                        }
                        if stale {
                            span { class: "badge amber", title: "The form changed after these rows were read.", "Stale" }
                        }
                    },
                    actions: rsx! {
                        button {
                            class: "btn sm",
                            disabled: shown.is_none(),
                            title: "Draw this result and its query geometry on the map",
                            onclick: show_on_map,
                            Ico { icon: Icon::MapPin, size: 16 }
                            "Show on map"
                        }
                        button {
                            class: "btn sm",
                            disabled: true,
                            title: "Export has no design yet: the scope (selection, result, or all matches) and the format are not chosen. Open in Query carries the SQL.",
                            Ico { icon: Icon::Download, size: 16 }
                            "Export"
                        }
                    },
                    ResultsBody {
                        key_id: key.clone(),
                        result: shown.clone(),
                        running,
                        error: d.error.clone(),
                        stale,
                        selected: d.selected.clone(),
                    }
                }
            }
            ResizeHandle { surface: SURFACE, target: "query", size: QUERY_SIZE, side: Side::After }
            Panel { surface: SURFACE, id: "query", title: "Query", size: QUERY_SIZE,
                QueryBody { sql, source: source.clone(), key_id: key.clone(), open: d.source_open }
            }
        }
    }
}
