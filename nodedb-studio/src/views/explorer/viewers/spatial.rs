//! Spatial storage-mode viewer: a static map sketch.
//!
//! Placeholder SVG only. A production build would integrate a real map widget
//! (e.g. MapLibre or Leaflet) — not chosen here.

use dioxus::prelude::*;

#[component]
pub fn SpatialViewer() -> Element {
    rsx! {
        div { class: "engine-viewer",
            div { class: "spatial-map",
                svg { view_box: "0 0 800 400", style: "width:100%; height:100%;",
                    path {
                        d: "M50 220 Q 120 180 200 200 T 350 180 T 500 210 T 650 190 L 750 200 L 750 250 Q 650 270 500 260 T 350 270 T 200 250 T 50 270 Z",
                        fill: "rgba(24,95,165,0.10)", stroke: "rgba(24,95,165,0.4)", stroke_width: "0.7",
                    }
                    circle { cx: "180", cy: "210", r: "4", fill: "#1a1a18" }
                    circle { cx: "240", cy: "195", r: "4", fill: "#1a1a18" }
                    circle { cx: "310", cy: "220", r: "4", fill: "#1a1a18" }
                    circle { cx: "420", cy: "200", r: "4", fill: "#1a1a18" }
                    circle { cx: "510", cy: "230", r: "4", fill: "#1a1a18" }
                    circle { cx: "600", cy: "210", r: "4", fill: "#1a1a18" }
                    circle { cx: "680", cy: "220", r: "4", fill: "#1a1a18" }
                }
                div { style: "position:absolute; top:14px; left:14px; padding: 6px 10px; background: var(--bg-primary); border: 0.5px solid var(--border-mid); border-radius: 6px; font-family: var(--font-mono); font-size: 11px;",
                    "2,108 features · WGS84 · bbox 49.0,-8.6 → 60.8,1.8"
                }
            }
        }
    }
}
