//! Graph Explorer → Cypher: an editor above the canvas, the selection beside
//! them, and the source graph's totals in the toolbar.
//!
//! The toolbar's Layout arranges graph nodes only. Panels dock through the
//! workspace, never through it. Whole-graph totals come from the binding.
//! The visible result's counts sit on the canvas.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::resource_state;
use crate::components::code_editor::CodeEditor;
use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::models::console::Protocol;
use crate::models::cypher::parse;
use crate::models::graph_layout::LayoutKind;
use crate::routes::tabs::GraphTab;
use crate::services::backend::Backend;
use crate::state::graph::GraphUi;
use crate::state::workspace::{Surface, Workspace};
use crate::views::graph::inspector::{Flavor, GraphInspector};
use crate::views::graph::result::ResultCanvas;
use crate::views::graph::toolbar::{
    BindingSelect, LayoutSelect, Totals, catalog_resource, parse_choice,
};

const SURFACE: Surface = Surface::Graph(GraphTab::Cypher);
const QUERY_SIZE: PanelSize = PanelSize::height(190.0, 130.0, 480.0);
const SELECTION_SIZE: PanelSize = PanelSize::width(300.0, 260.0, 520.0);
const LIMITS: [usize; 5] = [10, 25, 50, 100, 250];

#[component]
pub fn CypherTab() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let layout_state = use_context::<Signal<Workspace>>();
    let graph = use_context::<GraphUi>();
    let mut ui = graph.cypher;
    let mut canvas = graph.cypher_canvas;
    let request = use_memo(move || ui.read().request.clone());

    let mut run = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let _conn = conn();
            let r = request();
            async move { backend.run_cypher(&r.binding, &r.query, r.limit).await }
        })
    };
    let state = resource_state(&run);
    let catalog = catalog_resource();

    let (text, binding, limit, layout) = {
        let u = ui.read();
        (u.text.clone(), u.binding.clone(), u.limit, u.layout)
    };
    let issue = parse(&text).err().map(|e| e.to_string());
    let editor_open = layout_state.read().is_visible(SURFACE, "query");

    rsx! {
        div { class: "page-body",
            div { class: "toolbar nowrap graph-toolbar",
                BindingSelect {
                    value: binding.clone(),
                    on_change: move |name: String| ui.write().binding = name,
                }
                Totals { binding: binding.clone() }
                div { class: "toolbar-spacer" }
                LayoutSelect {
                    value: layout,
                    on_change: move |kind: LayoutKind| ui.write().layout = kind,
                }
                select {
                    class: "select native compact",
                    "aria-label": "Limit",
                    value: "{limit}",
                    onchange: move |e| {
                        if let Some(n) = parse_choice::<usize>(&e.value()) { ui.write().limit = n; }
                    },
                    for n in LIMITS {
                        option { key: "{n}", value: "{n}", selected: n == limit, "Limit: {n}" }
                    }
                }
                button {
                    class: "btn primary",
                    disabled: issue.is_some(),
                    title: if let Some(why) = issue.as_ref() { "{why}" } else { "Run (Ctrl+Enter)" },
                    onclick: move |_| {
                        ui.write().run();
                        canvas.write().reset();
                    },
                    Ico { icon: Icon::Play, size: 16 }
                    "Run"
                }
            }
            div { class: "workbench",
                div { class: "workbench col graph-main",
                    Panel {
                        surface: SURFACE,
                        id: "query",
                        title: "Query",
                        size: QUERY_SIZE,
                        CodeEditor {
                            class: "graph-code",
                            value: text,
                            protocol: Protocol::Cypher,
                            label: "Cypher query",
                            placeholder: "MATCH (a)-[:RELATION]->(b) RETURN a, b",
                            on_input: move |t: String| ui.write().text = t,
                            on_run: move |_| {
                                if parse(&ui.peek().text).is_ok() {
                                    ui.write().run();
                                    canvas.write().reset();
                                }
                            },
                        }
                        if let Some(why) = issue.as_ref() {
                            div { class: "graph-issue", role: "alert",
                                Ico { icon: Icon::CircleAlert, size: 14 }
                                "{why}"
                            }
                        }
                    }
                    if editor_open {
                        ResizeHandle { surface: SURFACE, target: "query", size: QUERY_SIZE, side: Side::Before }
                    }
                    ResultCanvas {
                        state: state.clone(),
                        layout,
                        canvas,
                        on_retry: move |_| run.restart(),
                        empty_message: "The query matched no nodes. Check its labels and filters.",
                    }
                }
                ResizeHandle { surface: SURFACE, target: "selection", size: SELECTION_SIZE, side: Side::After }
                Panel {
                    surface: SURFACE,
                    id: "selection",
                    title: "Selection",
                    size: SELECTION_SIZE,
                    if let Some(result) = state.loaded() {
                        GraphInspector {
                            result: result.clone(),
                            canvas,
                            binding: request().binding,
                            flavor: Flavor::Selection,
                            catalog: catalog.clone(),
                        }
                    } else {
                        div { class: "inspector-note padded", "Run a query to select a node or an edge." }
                    }
                }
            }
        }
    }
}
