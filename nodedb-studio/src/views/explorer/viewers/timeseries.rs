//! Timeseries storage-mode viewer: a static line-chart sketch.
//!
//! Placeholder SVG only. A production build would integrate a real chart
//! library (e.g. Plotly or Chart.js) — not chosen here.

use dioxus::prelude::*;

#[component]
pub fn TimeseriesViewer() -> Element {
    rsx! {
        div { class: "engine-viewer",
            div { class: "ts-chart", style: "height: 100%;",
                svg { view_box: "0 0 800 300", preserve_aspect_ratio: "none", style: "width:100%; height:100%;",
                    polyline {
                        points: "0,180 50,160 100,170 150,140 200,150 250,120 300,130 350,100 400,110 450,80 500,90 550,60 600,75 650,55 700,68 750,40 800,55",
                        stroke: "#185fa5", stroke_width: "1.5", fill: "none",
                    }
                    polyline {
                        points: "0,220 50,210 100,225 150,200 200,215 250,195 300,205 350,185 400,195 450,170 500,185 550,160 600,175 650,155 700,170 750,140 800,160",
                        stroke: "#3b6d11", stroke_width: "1.5", fill: "none",
                    }
                }
            }
        }
    }
}
