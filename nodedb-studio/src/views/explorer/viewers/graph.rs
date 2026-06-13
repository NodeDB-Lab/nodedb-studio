//! Graph storage-mode viewer: a small static node-link sketch.
//!
//! Placeholder SVG only. A production build would integrate a real graph
//! renderer (e.g. Cytoscape or sigma.js) — not chosen here.

use dioxus::prelude::*;

#[component]
pub fn GraphViewer() -> Element {
    rsx! {
        div { class: "engine-viewer",
            div { class: "graph-mini", style: "height: 100%;",
                svg { view_box: "0 0 600 400", preserve_aspect_ratio: "xMidYMid meet", style: "width:100%; height:100%;",
                    path { class: "gx-edge", d: "M150,200 L300,120" }
                    path { class: "gx-edge", d: "M150,200 L300,280" }
                    path { class: "gx-edge", d: "M300,120 L450,200" }
                    path { class: "gx-edge", d: "M300,280 L450,200" }
                    path { class: "gx-edge", d: "M450,200 L520,140" }
                    path { class: "gx-edge", d: "M300,120 L300,280" }
                    g { class: "gx-node", circle { cx: "150", cy: "200", r: "22", fill: "#185fa5" } text { x: "150", y: "204", fill: "white", "U" } }
                    g { class: "gx-node", circle { cx: "300", cy: "120", r: "18", fill: "#3b6d11" } text { x: "300", y: "124", fill: "white", "P" } }
                    g { class: "gx-node", circle { cx: "300", cy: "280", r: "18", fill: "#3b6d11" } text { x: "300", y: "284", fill: "white", "P" } }
                    g { class: "gx-node", circle { cx: "450", cy: "200", r: "22", fill: "#185fa5" } text { x: "450", y: "204", fill: "white", "U" } }
                    g { class: "gx-node", circle { cx: "520", cy: "140", r: "14", fill: "#854f0b" } text { x: "520", y: "144", fill: "white", "T" } }
                }
            }
        }
    }
}
