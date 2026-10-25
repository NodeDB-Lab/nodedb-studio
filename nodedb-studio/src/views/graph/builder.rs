//! Graph Explorer → Builder: a pattern form that writes Cypher, the result
//! graph, and the selected node's details.
//!
//! The form and the query that runs are the same text: the preview shows
//! exactly what Run sends.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::resource_state;
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::models::graph::GraphResult;
use crate::models::graph_layout::LayoutKind;
use crate::routes::tabs::GraphTab;
use crate::services::backend::Backend;
use crate::state::graph::GraphUi;
use crate::state::workspace::Surface;
use crate::views::graph::builder_form::BuilderForm;
use crate::views::graph::inspector::{Flavor, GraphInspector};
use crate::views::graph::result::ResultCanvas;
use crate::views::graph::toolbar::{BindingSelect, LayoutSelect, Totals, catalog_resource};

const SURFACE: Surface = Surface::Graph(GraphTab::Builder);
const BUILD_SIZE: PanelSize = PanelSize::width(360.0, 300.0, 520.0);
const DETAILS_SIZE: PanelSize = PanelSize::width(380.0, 280.0, 560.0);

#[component]
pub fn BuilderTab() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let graph = use_context::<GraphUi>();
    let mut ui = graph.builder;
    let canvas = graph.builder_canvas;
    let request = use_memo(move || ui.read().request.clone());

    let mut run = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let _conn = conn();
            let r = request();
            async move {
                match r {
                    Some(r) => backend.run_cypher(&r.binding, &r.query, r.limit).await,
                    None => Ok(GraphResult::default()),
                }
            }
        })
    };
    let state = resource_state(&run);
    let catalog = catalog_resource();
    let (binding, layout) = {
        let u = ui.read();
        (u.binding.clone(), u.layout)
    };

    rsx! {
        div { class: "page-body",
            div { class: "toolbar nowrap graph-toolbar",
                span { class: "toolbar-label", "Source" }
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
            }
            div { class: "workbench",
                Panel {
                    surface: SURFACE,
                    id: "build",
                    title: "Build query",
                    size: BUILD_SIZE,
                    BuilderForm { ui, canvas }
                }
                ResizeHandle { surface: SURFACE, target: "build", size: BUILD_SIZE, side: Side::Before }
                Panel {
                    surface: SURFACE,
                    id: "graph",
                    title: "Graph view",
                    class: "fill",
                    ResultCanvas {
                        state: state.clone(),
                        layout,
                        canvas,
                        counts: true,
                        on_retry: move |_| run.restart(),
                        empty_message: "No path matches this pattern. Loosen a filter or raise the depth.",
                    }
                }
                ResizeHandle { surface: SURFACE, target: "details", size: DETAILS_SIZE, side: Side::After }
                Panel {
                    surface: SURFACE,
                    id: "details",
                    title: "Node details",
                    size: DETAILS_SIZE,
                    if let Some(result) = state.loaded() {
                        GraphInspector {
                            result: result.clone(),
                            canvas,
                            binding: request().map(|r| r.binding).unwrap_or(binding),
                            flavor: Flavor::Details,
                            catalog: catalog.clone(),
                        }
                    } else {
                        div { class: "inspector-note padded", "Run the query to select a node or an edge." }
                    }
                }
            }
        }
    }
}
