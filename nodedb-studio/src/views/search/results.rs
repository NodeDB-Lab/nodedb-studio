//! The Search tab: the query bar, the ranked records, and the selected
//! record's details.
//!
//! A search runs only when Search is pressed. Changing the query marks the
//! shown result stale and never replaces it. A failed or canceled run keeps
//! the result before it.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::models::search::{SearchCollection, SearchMode, SearchSetup};
use crate::models::search_form::{Issue, SearchForm};
use crate::routes::tabs::SearchTab;
use crate::services::backend::Backend;
use crate::state::search::{ResultsUi, SearchUi};
use crate::state::workspace::Surface;
use crate::views::search::compare::CompareSheet;
use crate::views::search::hit_details::DetailsBody;
use crate::views::search::page::resolve;
use crate::views::search::query_bar::QueryBar;
use crate::views::search::ranked::{RankedBody, ViewToggle};

const SURFACE: Surface = Surface::Search(SearchTab::Results);
const DETAILS_SIZE: PanelSize = PanelSize::width(440.0, 340.0, 620.0);

#[component]
pub fn ResultsTab(setup: SearchSetup) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let ui = use_context::<SearchUi>();
    let mut r = ui.results;

    // Typing changes the form, not this memo, so only a new search restarts
    // the read.
    let pending = use_memo(move || r.read().pending.clone());
    let run = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let _conn = conn();
            let pending = pending();
            async move {
                match pending {
                    Some((seq, req)) => Some((seq, backend.search_run(&req).await)),
                    None => None,
                }
            }
        })
    };
    // A finished search lands in the state. A late answer to a search that
    // was canceled or replaced no longer matches `pending` and is dropped.
    use_effect(move || {
        let Some(Some((seq, outcome))) = run.read().clone() else {
            return;
        };
        let mut s = r.write();
        if s.pending.as_ref().map(|p| p.0) != Some(seq) {
            return;
        }
        s.pending = None;
        match outcome {
            Ok(result) => {
                // Reading a result opens its first record, unless the one
                // already open is still in it.
                let keep = s
                    .selected
                    .as_deref()
                    .is_some_and(|id| result.hit(id).is_some());
                if !keep {
                    s.selected = result.hits.first().map(|h| h.id.clone());
                }
                s.shown = Some(result);
                s.error = None;
                s.cancelled = false;
            }
            Err(e) => s.error = Some(e.to_string()),
        }
    });

    let Some(collection) = resolve(&setup, r.read().collection.as_deref()).cloned() else {
        return rsx! {
            div { class: "empty-state",
                h2 { "No collection chosen" }
                p { "Choose a collection to search." }
            }
        };
    };
    let collection_name = collection.name.clone();

    let state = r.read().clone();
    let mut form = state.form.clone();
    form.settle(&collection);
    let checked = form.check(&collection);
    let issues: Vec<Issue> = checked.clone().err().unwrap_or_default();
    let running = state.pending.is_some();
    let stale = match (&state.shown, &checked) {
        (Some(shown), Ok(req)) => shown.request != *req,
        (Some(_), Err(_)) => true,
        (None, _) => false,
    };
    let selected = state
        .shown
        .as_ref()
        .and_then(|s| {
            state
                .selected
                .as_deref()
                .and_then(|id| s.hit(id))
                .or_else(|| s.hits.first())
        })
        .map(|h| h.id.clone());
    let shown_count = state.shown.as_ref().map(|s| s.hits.len());
    let limit = state.shown.as_ref().map(|s| s.request.limit);

    rsx! {
        QueryBar {
            setup: setup.clone(),
            collection: collection.clone(),
            form: form.clone(),
            issues: issues.clone(),
            running,
            shown: state.shown.clone(),
            stale,
            on_run: {
                let (collection, form) = (collection.clone(), form.clone());
                move |_| run_search(r, &collection, &form)
            },
        }
        div { class: "workbench srch-results",
            Panel {
                surface: SURFACE,
                id: "results",
                title: "Ranked results",
                class: "fill",
                actions: rsx! { ViewToggle {} },
                title_extra: rsx! {
                    if let (Some(n), Some(l), Some(shown)) = (shown_count, limit, state.shown.as_ref()) {
                        span { class: "srch-count", title: "{shown.ranking_note()}", "{n} shown · limit {l}" }
                    }
                    if stale {
                        span { class: "badge amber", "Stale" }
                    }
                },
                RankedBody {
                    state: state.clone(),
                    stale,
                    compare_blocked: compare_blocked(&collection, &form),
                    selected: selected.clone(),
                    on_cancel: move |_| {
                        let mut s = r.write();
                        s.pending = None;
                        s.cancelled = true;
                    },
                    on_open_compare: {
                        let (collection, form) = (collection.clone(), form.clone());
                        move |_| open_compare(r, &collection, &form)
                    },
                }
            }
            ResizeHandle { surface: SURFACE, target: "details", size: DETAILS_SIZE, side: Side::After }
            Panel { surface: SURFACE, id: "details", title: "Result details", size: DETAILS_SIZE,
                DetailsBody {
                    collection: collection.clone(),
                    state: state.clone(),
                    selected,
                }
            }
        }
        if state.compare.is_some() {
            CompareSheet { collection_name: collection_name.clone() }
        }
    }
}

/// Why Compare modes cannot run: each mode needs its own inputs.
fn compare_blocked(collection: &SearchCollection, form: &SearchForm) -> Option<String> {
    for mode in SearchMode::ALL {
        let mut f = form.clone();
        f.mode = mode;
        if let Err(issues) = f.check(collection) {
            let why = issues
                .first()
                .map_or("The inputs are incomplete.", |i| i.message.as_str());
            return Some(format!("Compare modes runs {} too. {why}", mode.label()));
        }
    }
    None
}

/// Start a search from the form, or mark the form as tried so it shows
/// what is missing.
fn run_search(mut r: Signal<ResultsUi>, collection: &SearchCollection, form: &SearchForm) {
    let mut s = r.write();
    match form.check(collection) {
        Ok(req) => {
            s.seq += 1;
            s.pending = Some((s.seq, req));
            s.error = None;
            s.cancelled = false;
            s.tried = false;
        }
        Err(_) => s.tried = true,
    }
}

/// Open the Compare modes sheet on the form's query, in all three modes.
fn open_compare(mut r: Signal<ResultsUi>, collection: &SearchCollection, form: &SearchForm) {
    // Each mode runs with the form's source, filters, and limit.
    let mut requests = Vec::new();
    for mode in SearchMode::ALL {
        let mut f = form.clone();
        f.mode = mode;
        match f.check(collection) {
            Ok(req) => requests.push(req),
            Err(_) => return,
        }
    }
    let Some(hybrid) = requests.pop() else { return };
    let mut s = r.write();
    let mut c = s.compare.take().unwrap_or_default();
    c.seq += 1;
    c.pending = Some((c.seq, hybrid));
    c.error = None;
    s.compare = Some(c);
}
