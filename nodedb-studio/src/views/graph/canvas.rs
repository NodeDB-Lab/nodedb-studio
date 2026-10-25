//! The graph canvas: an SVG drawn from typed graph data. Placement and edge
//! curves come from `models::graph_layout` and `models::graph_scene`, so
//! nothing here decides where a node goes.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::graph::{GraphPath, GraphResult, NodeKind};
use crate::models::graph_layout::{LayoutKind, layout};
use crate::models::graph_scene::build;
use crate::state::graph::{CanvasState, Pick};
use crate::views::graph::glyphs::{LegendShape, NodeShape, R, kind_class};

/// The smallest scale at 100% zoom: with 14px and larger label text, 12px stays
/// the floor. A larger graph scrolls.
const MIN_SCALE: f32 = 0.86;

#[component]
pub fn GraphCanvas(
    /// What to draw: the result with everything merged into it.
    graph: GraphResult,
    layout_kind: LayoutKind,
    canvas: Signal<CanvasState>,
    selected: Option<Pick>,
    /// The path drawn in the accent color.
    highlight: Option<GraphPath>,
    /// Show how many nodes of each kind the legend covers.
    #[props(default = false)]
    counts: bool,
    /// Called after a click or key press selects something.
    #[props(default)]
    on_pick: EventHandler<Pick>,
) -> Element {
    let placed = layout(&graph, layout_kind);
    let scene = build(&graph, &placed);
    let zoom = canvas.read().zoom;
    let (vx, vy, vw, vh) = scene.view_box;
    let kinds: Vec<NodeKind> = graph.kinds();
    let hot_node = |p: &str| {
        highlight
            .as_ref()
            .is_some_and(|h| h.nodes.iter().any(|n| n == p))
    };
    let hot_edge = |id: &str| {
        highlight
            .as_ref()
            .is_some_and(|h| h.edges.iter().any(|e| e == id))
    };
    let picked_node = match &selected {
        Some(Pick::Node(p)) => Some(p.clone()),
        _ => None,
    };
    let picked_edge = match &selected {
        Some(Pick::Edge(e)) => Some(e.clone()),
        _ => None,
    };
    // One canvas unit is one pixel at 100%, so labels never shrink below
    // their size. A graph smaller than the panel scales up to fill it.
    let z = f32::from(zoom) / 100.0 * MIN_SCALE;
    let svg_size = format!(
        "width: max(100%, {:.0}px); height: max(100%, {:.0}px);",
        vw * z,
        vh * z
    );
    let summary = format!("{} nodes · {} edges", graph.nodes.len(), graph.edges.len());

    rsx! {
        div { class: "graph-canvas",
            div { class: "graph-scroll",
                svg {
                    class: "graph-svg",
                    role: "img",
                    "aria-label": "Graph of {summary}",
                    view_box: "{vx} {vy} {vw} {vh}",
                    style: "{svg_size}",
                    defs {
                        marker {
                            id: "g-arrow",
                            class: "g-arrow",
                            view_box: "0 0 10 10",
                            ref_x: "9",
                            ref_y: "5",
                            marker_width: "8",
                            marker_height: "8",
                            orient: "auto",
                            path { d: "M0 1 L10 5 L0 9 Z" }
                        }
                        marker {
                            id: "g-arrow-hot",
                            class: "g-arrow hot",
                            view_box: "0 0 10 10",
                            ref_x: "9",
                            ref_y: "5",
                            marker_width: "8",
                            marker_height: "8",
                            orient: "auto",
                            path { d: "M0 1 L10 5 L0 9 Z" }
                        }
                    }
                    for e in scene.edges.iter() {
                        {
                            let id = e.id.clone();
                            let hot = hot_edge(&e.id);
                            let on = picked_edge.as_deref() == Some(e.id.as_str());
                            let class = match (hot, on) {
                                (_, true) => "g-edge picked",
                                (true, false) => "g-edge hot",
                                (false, false) => "g-edge",
                            };
                            let arrow = if hot || on { "url(#g-arrow-hot)" } else { "url(#g-arrow)" };
                            rsx! {
                                g { key: "{e.id}", class: "g-edge-group",
                                    path { class: "{class}", d: "{e.d}", marker_end: "{arrow}", "data-relation": "{e.relation}" }
                                    path {
                                        class: "g-edge-hit",
                                        d: "{e.d}",
                                        onclick: move |_| {
                                            canvas.write().select(Pick::Edge(id.clone()));
                                            on_pick.call(Pick::Edge(id.clone()));
                                        },
                                    }
                                    text {
                                        class: if hot || on { "g-edge-label hot" } else { "g-edge-label" },
                                        x: "{e.label_x}",
                                        y: "{e.label_y}",
                                        "{e.relation}"
                                    }
                                }
                            }
                        }
                    }
                    for n in scene.nodes.iter() {
                        {
                            let path = n.path.clone();
                            let key_path = n.path.clone();
                            let on = picked_node.as_deref() == Some(n.path.as_str());
                            let hot = hot_node(&n.path);
                            let class = format!(
                                "g-node {}{}{}",
                                kind_class(n.kind),
                                if on { " picked" } else { "" },
                                if hot { " hot" } else { "" },
                            );
                            let label = format!("{} {}, {}", n.kind.label(), n.label, n.sub);
                            rsx! {
                                g {
                                    key: "{n.path}",
                                    class: "{class}",
                                    transform: "translate({n.x} {n.y})",
                                    role: "button",
                                    tabindex: "0",
                                    "aria-label": "{label}",
                                    "aria-pressed": "{on}",
                                    onclick: move |_| {
                                        canvas.write().select(Pick::Node(path.clone()));
                                        on_pick.call(Pick::Node(path.clone()));
                                    },
                                    onkeydown: move |e| {
                                        if e.key() == Key::Enter || e.key() == Key::Character(" ".into()) {
                                            e.prevent_default();
                                            canvas.write().select(Pick::Node(key_path.clone()));
                                            on_pick.call(Pick::Node(key_path.clone()));
                                        }
                                    },
                                    if on {
                                        circle { class: "g-ring outer", r: "{R + 9.0}" }
                                        circle { class: "g-ring", r: "{R + 5.0}" }
                                    }
                                    NodeShape { kind: n.kind, r: R }
                                    text { class: "g-label", y: "{R + 20.0}", "{n.label}" }
                                    text { class: "g-sub", y: "{R + 35.0}", "{n.sub}" }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "graph-legend",
                for k in kinds.iter() {
                    span { key: "{k.label()}", class: "legend-item",
                        LegendShape { kind: *k }
                        if counts {
                            {format!("{} ({})", k.label(), graph.nodes.iter().filter(|n| n.kind == *k).count())}
                        } else {
                            "{k.label()}"
                        }
                    }
                }
            }
            div { class: "graph-zoom",
                button {
                    class: "btn icon-only",
                    title: "Zoom out",
                    "aria-label": "Zoom out",
                    onclick: move |_| canvas.write().zoom_out(),
                    Ico { icon: Icon::Minus, size: 16 }
                }
                span { class: "zoom-level", "{zoom}%" }
                button {
                    class: "btn icon-only",
                    title: "Zoom in",
                    "aria-label": "Zoom in",
                    onclick: move |_| canvas.write().zoom_in(),
                    Ico { icon: Icon::Plus, size: 16 }
                }
                button {
                    class: "btn",
                    title: "Fit the graph to the view",
                    onclick: move |_| canvas.write().fit(),
                    "Fit"
                }
            }
            div { class: "graph-count",
                "{summary}"
                if graph.truncated {
                    " · more match"
                }
            }
        }
    }
}
