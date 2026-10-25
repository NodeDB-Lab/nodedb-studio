//! The Analyze tab: run text through the analyzer an index is bound to and
//! read the tokens, their source spans, and each processing stage.
//!
//! The server supplies every value. A test never changes a collection, an
//! analyzer, or an index, and the bound analyzer shows read-only: its binding
//! changes in the Designer.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::models::search::SearchSetup;
use crate::models::search_analyze::AnalyzeRequest;
use crate::routes::tabs::SearchTab;
use crate::services::backend::Backend;
use crate::state::designer::DesignerState;
use crate::state::search::{AnalyzeUi, SearchUi};
use crate::state::workspace::Surface;
use crate::views::search::analyze_bodies::{DetailsBody, InputBody, TokensBody, TraceBody};
use crate::views::search::page::resolve;
use crate::views::search::tasks::open_index_in_designer;

const SURFACE: Surface = Surface::Search(SearchTab::Analyze);
const INPUT_SIZE: PanelSize = PanelSize::height(236.0, 190.0, 420.0);
const TRACE_SIZE: PanelSize = PanelSize::height(212.0, 150.0, 400.0);
const DETAILS_SIZE: PanelSize = PanelSize::width(450.0, 340.0, 620.0);

fn edit(mut a: Signal<AnalyzeUi>, f: impl FnOnce(&mut AnalyzeUi)) {
    f(&mut a.write());
}

#[component]
pub fn AnalyzeTab(setup: SearchSetup) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let ui = use_context::<SearchUi>();
    let designer = use_context::<Signal<DesignerState>>();
    let nav = use_navigator();
    let a = ui.analyze;

    let pending = use_memo(move || a.read().pending.clone());
    let run = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let _conn = conn();
            let pending = pending();
            async move {
                match pending {
                    Some((seq, req)) => Some((seq, backend.search_analyze(&req).await)),
                    None => None,
                }
            }
        })
    };
    // A late answer to a test that was replaced no longer matches `pending`.
    use_effect(move || {
        let Some(Some((seq, outcome))) = run.read().clone() else {
            return;
        };
        edit(a, |s| {
            if s.pending.as_ref().map(|p| p.0) != Some(seq) {
                return;
            }
            s.pending = None;
            match outcome {
                Ok(done) => {
                    s.shown = Some(done);
                    s.error = None;
                    s.token = None;
                }
                Err(e) => s.error = Some(e.to_string()),
            }
        });
    });

    let Some(collection) = resolve(&setup, a.read().collection.as_deref()).cloned() else {
        return rsx! {
            div { class: "empty-state", h2 { "No collection chosen" } }
        };
    };

    let Some(text) = collection.text.clone() else {
        return rsx! {
            div { class: "empty-state",
                Ico { icon: Icon::Search, size: 24 }
                h2 { "No full-text index" }
                p {
                    "{collection.name} has no full-text index, so it has no analyzer to test. Add one in the Designer."
                }
            }
        };
    };

    let state = a.read().clone();
    let index = text.index.clone();
    let field = state
        .field
        .clone()
        .filter(|f| text.fields.contains(f))
        .or_else(|| text.fields.iter().find(|f| f.as_str() == "body").cloned())
        .or_else(|| text.fields.first().cloned())
        .unwrap_or_default();
    let request = AnalyzeRequest {
        collection: collection.name.clone(),
        index: index.clone(),
        field: field.clone(),
        text: state.text.clone(),
        role: state.role,
    };
    let ready = !state.text.trim().is_empty();
    let stale = state.shown.as_ref().is_some_and(|s| s.request != request);
    let running = state.pending.is_some();
    let names: Vec<String> = setup.collections.iter().map(|c| c.name.clone()).collect();

    rsx! {
        div { class: "toolbar nowrap srch-bar",
            label { r#for: "srch-an-collection", class: "toolbar-label", "Collection" }
            select {
                id: "srch-an-collection",
                class: "select native",
                onchange: move |e| {
                    let name = e.value();
                    edit(a, |s| *s = AnalyzeUi { collection: Some(name), ..AnalyzeUi::default() });
                },
                for c in names.iter() {
                    option { key: "{c}", value: "{c}", selected: *c == collection.name, "{c}" }
                }
            }
            label { r#for: "srch-an-index", class: "toolbar-label", "Index" }
            select {
                id: "srch-an-index",
                class: "select native",
                option { key: "{index}", value: "{index}", selected: true, "{index}" }
            }
            label { r#for: "srch-an-field", class: "toolbar-label", "Field" }
            select {
                id: "srch-an-field",
                class: "select native",
                onchange: move |e| {
                    let v = e.value();
                    edit(a, |s| s.field = Some(v));
                },
                for f in text.fields.iter() {
                    option { key: "{f}", value: "{f}", selected: *f == field, "{f}" }
                }
            }
            label { r#for: "srch-an-profile", class: "toolbar-label", "Profile" }
            input {
                id: "srch-an-profile",
                class: "input srch-profile",
                readonly: true,
                disabled: true,
                value: "{text.analyzer} (bound)",
                title: "The analyzer bound to the index. Change the binding in the Designer.",
            }
            div { class: "toolbar-spacer" }
            button {
                class: "btn",
                title: "Open {index} in the Designer, where its analyzer binding changes",
                onclick: {
                    let index = index.clone();
                    move |_| open_index_in_designer(designer, nav, &index)
                },
                Ico { icon: Icon::ExternalLink, size: 16 }
                "Open in Designer"
            }
        }
        div { class: "workbench srch-analyze",
            div { class: "workbench col srch-an-left",
                Panel { surface: SURFACE, id: "input", title: "Input text", size: INPUT_SIZE,
                    InputBody {
                        text: state.text.clone(),
                        role: state.role,
                        ready,
                        running,
                        on_run: {
                            let request = request.clone();
                            move |_| {
                                if !ready { return; }
                                edit(a, |s| {
                                    s.seq += 1;
                                    s.pending = Some((s.seq, request.clone()));
                                    s.error = None;
                                });
                            }
                        },
                    }
                }
                ResizeHandle { surface: SURFACE, target: "input", size: INPUT_SIZE, side: Side::Before }
                Panel {
                    surface: SURFACE,
                    id: "tokens",
                    title: "Analyzed tokens",
                    class: "fill",
                    title_extra: rsx! {
                        if stale { span { class: "badge amber", "Stale" } }
                    },
                    TokensBody { state: state.clone(), running, stale }
                }
                ResizeHandle { surface: SURFACE, target: "trace", size: TRACE_SIZE, side: Side::After }
                Panel { surface: SURFACE, id: "trace", title: "Processing trace", size: TRACE_SIZE,
                    TraceBody { state: state.clone() }
                }
            }
            ResizeHandle { surface: SURFACE, target: "details", size: DETAILS_SIZE, side: Side::After }
            Panel { surface: SURFACE, id: "details", title: "Analyzer details", size: DETAILS_SIZE,
                DetailsBody {
                    collection: collection.clone(),
                    text: text.clone(),
                    field: field.clone(),
                    shown: state.shown.clone(),
                    raw: state.text.clone(),
                }
            }
        }
    }
}
