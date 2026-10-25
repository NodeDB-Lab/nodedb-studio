//! Graph Explorer → GraphRAG: a question, an explicit retrieval source, the
//! evidence it finds, and the graph around that evidence.
//!
//! Retrieval and answer are different steps. This tab retrieves. The Answer
//! tab says what an answer needs and never writes one.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::components::tabs::TabBar;
use crate::models::graph_rag::{RagRequest, RagResult, RagSetup};
use crate::routes::tabs::GraphTab;
use crate::services::backend::Backend;
use crate::state::graph::{GraphUi, RagTab};
use crate::state::workspace::Surface;
use crate::views::graph::rag_answer::AnswerTab;
use crate::views::graph::rag_form::RagQueryForm;
use crate::views::graph::rag_panels::{ContextBody, EvidenceBody, SelectedBody};

const SURFACE: Surface = Surface::Graph(GraphTab::GraphRag);
const EVIDENCE_SIZE: PanelSize = PanelSize::width(380.0, 280.0, 560.0);
const SELECTED_SIZE: PanelSize = PanelSize::width(400.0, 300.0, 600.0);

#[component]
pub fn RagPage() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let graph = use_context::<GraphUi>();
    let mut ui = graph.rag;
    let mut canvas = graph.rag_canvas;

    let setup_read = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let _conn = conn();
            async move { backend.rag_setup().await }
        })
    };
    let setup_state = resource_state(&setup_read);
    let setup: Option<RagSetup> = setup_state.loaded().cloned();

    let request = use_memo(move || ui.read().request.clone());
    let mut search = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let _conn = conn();
            let r = request();
            async move {
                match r {
                    Some((_, r)) => backend.rag_search(&r).await,
                    None => Ok(RagResult::default()),
                }
            }
        })
    };
    let results = resource_state(&search);

    // The opening search: once the form has something to ask, ask it.
    {
        let setup = setup.clone();
        use_effect(move || {
            let u = ui.peek();
            if u.opened {
                return;
            }
            let Some(setup) = setup.as_ref() else { return };
            let Some(req) = u.form.request(setup) else {
                return;
            };
            let valid = req
                .issues(u.form.source(setup).map(|s| s.dimension))
                .is_empty();
            drop(u);
            if valid {
                let mut w = ui.write();
                w.opened = true;
                w.request = Some((1, req));
            }
        });
    }

    let (tab, form, selected) = {
        let u = ui.read();
        (u.tab, u.form.clone(), u.selected)
    };
    let asked: Option<RagRequest> = setup.as_ref().and_then(|s| form.request(s));
    let source_dim = setup
        .as_ref()
        .and_then(|s| form.source(s))
        .map(|s| s.dimension);
    let issues = asked
        .as_ref()
        .map(|r| r.issues(source_dim))
        .unwrap_or_else(|| vec!["The retrieval setup is not loaded.".to_string()]);
    let tabs: Vec<&'static str> = RagTab::ALL.iter().map(|t| t.label()).collect();
    let active = RagTab::ALL.iter().position(|t| *t == tab).unwrap_or(0);

    rsx! {
        div { class: "page-body",
            div { class: "toolbar nowrap graph-toolbar rag-head",
                span { class: "toolbar-note",
                    "Retrieve relevant records from a vector index, then the graph around them."
                }
                div { class: "toolbar-spacer" }
                TabBar {
                    class: "sub",
                    items: tabs,
                    active,
                    on_select: move |i: usize| {
                        if let Some(t) = RagTab::ALL.get(i) { ui.write().tab = *t; }
                    },
                }
            }
            match tab {
                RagTab::Retrieval => rsx! {
                    Panel {
                        surface: SURFACE,
                        id: "query",
                        title: "Query",
                        class: "rag-query",
                        AsyncView {
                            loading: setup_state.is_loading(),
                            empty: setup_state.is_empty(),
                            error: setup_state.error_message(),
                            retriable: setup_state.is_retriable(),
                            empty_message: "No collection has a vector index to retrieve from.",
                        }
                        if let Some(setup) = setup.clone() {
                            RagQueryForm {
                                ui,
                                setup,
                                issues: issues.clone(),
                                on_search: move |_| {
                                    if let Some(r) = asked.clone() {
                                        let mut w = ui.write();
                                        let seq = w.request.as_ref().map_or(0, |(n, _)| *n) + 1;
                                        w.request = Some((seq, r));
                                        w.selected = None;
                                        drop(w);
                                        canvas.write().reset();
                                    }
                                },
                            }
                        }
                    }
                    div { class: "workbench",
                        Panel {
                            surface: SURFACE,
                            id: "evidence",
                            title: "Retrieved evidence",
                            size: EVIDENCE_SIZE,
                            AsyncView {
                                loading: results.is_loading(),
                                empty: results.is_empty(),
                                error: results.error_message(),
                                retriable: results.is_retriable(),
                                on_retry: move |_| search.restart(),
                                empty_message: "Nothing in this index is close to the question.",
                            }
                            if let Some(r) = results.loaded() {
                                EvidenceBody {
                                    result: r.clone(),
                                    selected,
                                    on_select: move |rank: u32| ui.write().selected = Some(rank),
                                }
                            }
                        }
                        ResizeHandle { surface: SURFACE, target: "evidence", size: EVIDENCE_SIZE, side: Side::Before }
                        Panel {
                            surface: SURFACE,
                            id: "context",
                            title: "Graph neighbor context",
                            class: "fill",
                            if let Some(r) = results.loaded() {
                                ContextBody { result: r.clone(), ui, canvas }
                            }
                        }
                        ResizeHandle { surface: SURFACE, target: "selected", size: SELECTED_SIZE, side: Side::After }
                        Panel {
                            surface: SURFACE,
                            id: "selected",
                            title: "Selected evidence",
                            size: SELECTED_SIZE,
                            if let Some(r) = results.loaded() {
                                SelectedBody {
                                    result: r.clone(),
                                    selected,
                                    provider: setup.as_ref().and_then(|s| s.provider.clone()),
                                }
                            }
                        }
                    }
                },
                RagTab::Answer => rsx! {
                    AnswerTab {
                        provider: setup.as_ref().and_then(|s| s.provider.clone()),
                        evidence: results.loaded().map_or(0, |r| r.evidence.len()),
                        question: form.question.clone(),
                    }
                },
            }
        }
    }
}
