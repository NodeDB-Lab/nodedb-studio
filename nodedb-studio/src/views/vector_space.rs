//! Vector Space: a 2D projection scatter of a vector collection.
//! Placeholder SVG (no real projection/plot library).

use dioxus::prelude::*;

#[component]
pub fn VectorSpace() -> Element {
    // (fill, points) per cluster.
    let clusters: Vec<(&str, Vec<(i32, i32)>)> = vec![
        (
            "#185fa5",
            vec![
                (250, 200),
                (270, 220),
                (260, 180),
                (240, 210),
                (280, 190),
                (290, 230),
                (220, 195),
                (300, 215),
                (265, 240),
                (245, 170),
                (305, 195),
                (225, 225),
                (285, 170),
                (255, 155),
                (310, 175),
            ],
        ),
        (
            "#3b6d11",
            vec![
                (600, 280),
                (620, 300),
                (610, 260),
                (590, 290),
                (630, 270),
                (640, 310),
                (570, 275),
                (650, 295),
                (615, 320),
                (595, 250),
                (655, 275),
                (575, 305),
                (635, 250),
                (605, 235),
                (660, 255),
            ],
        ),
        (
            "#854f0b",
            vec![
                (430, 430),
                (450, 450),
                (440, 410),
                (420, 440),
                (460, 420),
                (470, 460),
                (400, 425),
                (480, 445),
                (445, 470),
                (425, 400),
                (485, 425),
                (405, 455),
            ],
        ),
        (
            "#9c2424",
            vec![
                (800, 150),
                (820, 170),
                (810, 130),
                (790, 160),
                (830, 140),
                (840, 180),
                (770, 145),
                (850, 165),
                (815, 190),
            ],
        ),
    ];
    rsx! {
        div { class: "view active",
            div { style: "height: 100%; display: grid; grid-template-rows: auto 1fr;",
                div { style: "padding: 10px 16px; background: var(--bg-secondary); border-bottom: 0.5px solid var(--border-mid); display: flex; align-items: center; gap: 10px;",
                    div { style: "font-family: var(--font-mono); font-size: 11px; color: var(--text-tertiary);", "doc_embeddings · 1.1M vectors · UMAP projection" }
                    div { style: "margin-left: auto; display: flex; gap: 6px;",
                        div { class: "segmented", button { class: "active", "2D" } button { "3D" } }
                        button { class: "btn small", "Color by: cluster" }
                        button { class: "btn small", "Sample: 5k" }
                        button { class: "btn small primary", "Find similar" }
                    }
                }
                div { class: "vec-canvas",
                    svg { view_box: "0 0 1000 600", preserve_aspect_ratio: "xMidYMid meet", style: "width:100%; height:100%;",
                        for (fill, points) in clusters {
                            g { fill: "{fill}", opacity: "0.7",
                                for (cx, cy) in points.iter().copied() {
                                    circle { cx: "{cx}", cy: "{cy}", r: "3" }
                                }
                            }
                        }
                        circle { cx: "265", cy: "200", r: "8", fill: "none", stroke: "var(--text-primary)", stroke_width: "2" }
                        circle { cx: "265", cy: "200", r: "40", fill: "none", stroke: "var(--text-primary)", stroke_width: "0.5", stroke_dasharray: "3,3" }
                    }
                    div { style: "position:absolute; top:14px; left:14px; padding: 10px 12px; background: var(--bg-primary); border: 0.5px solid var(--border-mid); border-radius: 6px; font-size: 11px; line-height: 1.5;",
                        div { class: "eyebrow", style: "margin-bottom:4px;", "Clusters" }
                        div { style: "display:flex; align-items:center; gap:6px;", span { style: "width:8px; height:8px; background:#185fa5; border-radius:50%" } " Docs · architecture" }
                        div { style: "display:flex; align-items:center; gap:6px;", span { style: "width:8px; height:8px; background:#3b6d11; border-radius:50%" } " Docs · graph" }
                        div { style: "display:flex; align-items:center; gap:6px;", span { style: "width:8px; height:8px; background:#854f0b; border-radius:50%" } " Docs · vector" }
                        div { style: "display:flex; align-items:center; gap:6px;", span { style: "width:8px; height:8px; background:#9c2424; border-radius:50%" } " Docs · ops" }
                    }
                }
            }
        }
    }
}
