//! The Search tab: ask for the nearest neighbors of a record or a vector,
//! read them ranked, and inspect one.
//!
//! A search runs only when Find similar is pressed. Changing the form marks
//! the shown result stale and never replaces it.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::models::vector::{NeighborRequest, VectorTarget};
use crate::routes::tabs::VectorTab;
use crate::services::backend::Backend;
use crate::state::vector::{Selection, VectorUi};
use crate::state::workspace::Surface;
use crate::views::vector::neighbors::NeighborsBody;
use crate::views::vector::record::{RecordMode, RecordPanel};
use crate::views::vector::search_form::QueryBody;

const SURFACE: Surface = Surface::Vector(VectorTab::Search);
const QUERY_SIZE: PanelSize = PanelSize::height(190.0, 140.0, 380.0);
const RECORD_SIZE: PanelSize = PanelSize::width(340.0, 280.0, 520.0);

#[component]
pub fn SearchTab(target: VectorTarget, provider: Option<String>) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let ui = use_context::<VectorUi>();
    let mut drafts = ui.drafts;
    let mut selected = ui.selected;
    let key = target.label();

    // Typing in the form changes the draft, not this memo, so only a new
    // search restarts the read.
    let pending = use_memo(use_reactive!(|key| drafts.read().search(&key).pending));
    let run = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let _conn = conn();
            let pending = pending();
            async move {
                match pending {
                    Some((seq, req)) => Some((seq, backend.vector_search(&req).await)),
                    None => None,
                }
            }
        })
    };

    // A finished search lands in the draft. A late answer to a search that
    // was canceled or replaced no longer matches `pending` and is dropped.
    use_effect(use_reactive!(|key| {
        let Some(Some((seq, outcome))) = run.read().clone() else {
            return;
        };
        let first = {
            let mut d = drafts.write();
            let st = d.search_mut(&key);
            if st.pending.as_ref().map(|p| p.0) != Some(seq) {
                return;
            }
            st.pending = None;
            match outcome {
                Ok(result) => {
                    let first = result.neighbors.first().map(|n| n.id.clone());
                    st.shown = Some(result);
                    st.error = None;
                    first
                }
                Err(e) => {
                    st.error = Some(e.to_string());
                    None
                }
            }
        };
        // Reading a result opens its closest record, without a new query.
        let own = selected.peek().as_ref().is_some_and(|s| s.target == key);
        if let (false, Some(id)) = (own, first) {
            selected.set(Some(Selection {
                target: key.clone(),
                id,
            }));
        }
    }));

    let s = drafts.read().search(&key);
    let form_request = s.form.check(&target).ok();
    let stale = match (&s.shown, &form_request) {
        (Some(shown), Some(req)) => shown.request != *req,
        (Some(_), None) => true,
        (None, _) => false,
    };
    let running = pending.read().is_some();
    let count = s.shown.as_ref().map(|r| r.neighbors.len());
    let sel = ui.selected_id(&key);

    rsx! {
        div { class: "workbench vec-search",
            div { class: "workbench col vec-left",
                Panel {
                    surface: SURFACE,
                    id: "query",
                    title: "Query",
                    size: QUERY_SIZE,
                    QueryBody {
                        target: target.clone(),
                        provider,
                        running,
                        on_run: {
                            let key = key.clone();
                            move |req: NeighborRequest| {
                                {
                                    let mut d = drafts.write();
                                    let st = d.search_mut(&key);
                                    st.seq += 1;
                                    st.pending = Some((st.seq, req));
                                    st.error = None;
                                }
                                selected.set(None);
                            }
                        },
                        on_cancel: {
                            let key = key.clone();
                            move |_| drafts.write().search_mut(&key).pending = None
                        },
                    }
                }
                ResizeHandle { surface: SURFACE, target: "query", size: QUERY_SIZE, side: Side::Before }
                Panel {
                    surface: SURFACE,
                    id: "neighbors",
                    title: "Neighbors",
                    class: "fill",
                    title_extra: rsx! {
                        if let Some(n) = count {
                            span { class: "vec-count", if n == 1 { "1 result" } else { "{n} results" } }
                        }
                        if stale {
                            span { class: "badge amber", "Stale" }
                        }
                    },
                    NeighborsBody {
                        metric: target.metric,
                        result: s.shown.clone(),
                        stale,
                        running,
                        error: s.error.clone(),
                        selected: sel,
                        on_select: {
                            let key = key.clone();
                            move |id: String| selected.set(Some(Selection { target: key.clone(), id }))
                        },
                    }
                }
            }
            ResizeHandle { surface: SURFACE, target: "record", size: RECORD_SIZE, side: Side::After }
            RecordPanel { surface: SURFACE, target: target.clone(), mode: RecordMode::Search, size: RECORD_SIZE }
        }
    }
}
