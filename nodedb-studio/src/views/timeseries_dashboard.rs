//! Timeseries dashboard: metric panels. Placeholder SVG charts (no real chart
//! library).

use dioxus::prelude::*;

#[component]
pub fn TimeseriesDashboard() -> Element {
    rsx! {
        div { class: "view active",
            div { style: "height: 100%; display: grid; grid-template-rows: auto 1fr;",
                div { style: "padding: 10px 16px; background: var(--bg-secondary); border-bottom: 0.5px solid var(--border-mid); display: flex; align-items: center; gap: 10px;",
                    div { style: "font-family: var(--font-mono); font-size: 11px; color: var(--text-tertiary);", "metrics · 48M points · auto-refresh 10s" }
                    div { style: "margin-left:auto; display:flex; gap:6px; align-items:center;",
                        div { class: "segmented",
                            button { "15m" } button { "1h" } button { class: "active", "6h" } button { "24h" } button { "7d" }
                        }
                        button { class: "btn small", "+ Panel" }
                        button { class: "btn small", "Save layout" }
                    }
                }
                div { style: "overflow:auto; padding: 16px; background: var(--bg-tertiary);",
                    div { style: "display:grid; grid-template-columns: 2fr 1fr; gap: 12px; margin-bottom: 12px;",
                        div { class: "card", style: "padding: 14px;",
                            div { style: "display:flex; justify-content:space-between; align-items:center; margin-bottom:10px;",
                                div {
                                    div { style: "font-size:13px; font-weight:600;", "Cluster QPS" }
                                    div { style: "font-family:var(--font-mono); font-size:10px; color:var(--text-tertiary);", "sum(rate(queries_total[1m]))" }
                                }
                                div { style: "font-family:var(--font-mono); font-size:18px; font-weight:500;", "12.4k " span { style: "font-size:11px; color:var(--text-success);", "▲ +8.2%" } }
                            }
                            div { style: "height: 200px;",
                                svg { view_box: "0 0 800 200", preserve_aspect_ratio: "none", style: "width:100%; height:100%;",
                                    defs {
                                        linearGradient { id: "qpsfill", x1: "0", y1: "0", x2: "0", y2: "1",
                                            stop { offset: "0", stop_color: "#185fa5", stop_opacity: "0.15" }
                                            stop { offset: "1", stop_color: "#185fa5", stop_opacity: "0" }
                                        }
                                    }
                                    path { d: "M0,140 L40,130 L80,135 L120,120 L160,125 L200,110 L240,100 L280,105 L320,90 L360,85 L400,95 L440,80 L480,70 L520,75 L560,60 L600,68 L640,50 L680,55 L720,40 L760,48 L800,42 L800,200 L0,200 Z", fill: "url(#qpsfill)" }
                                    polyline { points: "0,140 40,130 80,135 120,120 160,125 200,110 240,100 280,105 320,90 360,85 400,95 440,80 480,70 520,75 560,60 600,68 640,50 680,55 720,40 760,48 800,42", stroke: "#185fa5", stroke_width: "1.5", fill: "none" }
                                }
                            }
                        }
                        div { class: "card", style: "padding: 14px;",
                            div { style: "display:flex; justify-content:space-between; align-items:center; margin-bottom:10px;",
                                div {
                                    div { style: "font-size:13px; font-weight:600;", "p99 latency" }
                                    div { style: "font-family:var(--font-mono); font-size:10px; color:var(--text-tertiary);", "read · write" }
                                }
                                div { style: "font-family:var(--font-mono); font-size:18px; font-weight:500;", "22ms" }
                            }
                            div { style: "height: 200px;",
                                svg { view_box: "0 0 400 200", preserve_aspect_ratio: "none", style: "width:100%; height:100%;",
                                    polyline { points: "0,150 40,140 80,145 120,130 160,135 200,120 240,125 280,110 320,115 360,100 400,108", stroke: "#3b6d11", stroke_width: "1.5", fill: "none" }
                                    polyline { points: "0,100 40,95 80,90 120,85 160,82 200,78 240,75 280,72 320,68 360,65 400,62", stroke: "#854f0b", stroke_width: "1.5", fill: "none" }
                                }
                            }
                        }
                    }
                    div { style: "display:grid; grid-template-columns: 1fr 1fr 1fr; gap: 12px;",
                        TsPanel { title: "CPU utilization", points: "0,80 40,75 80,82 120,70 160,68 200,72 240,65 280,60 320,68 360,55 400,58", stroke: "#185fa5", caption: "avg 42% · max 78%" }
                        TsPanel { title: "Storage growth", points: "0,100 40,95 80,92 120,85 160,80 200,72 240,68 280,60 320,52 360,45 400,38", stroke: "#3b6d11", caption: "14.2 GB · +220MB/h" }
                        TsPanel { title: "Error rate", points: "0,108 40,110 80,105 120,108 160,102 200,90 240,95 280,98 320,100 360,102 400,98", stroke: "#9c2424", caption: "0.04% · 14 errors / 5m" }
                    }
                }
            }
        }
    }
}

#[component]
fn TsPanel(title: String, points: String, stroke: String, caption: String) -> Element {
    rsx! {
        div { class: "card", style: "padding:14px;",
            div { style: "font-size:13px; font-weight:600; margin-bottom:8px;", "{title}" }
            div { style: "height:120px;",
                svg { view_box: "0 0 400 120", preserve_aspect_ratio: "none", style: "width:100%; height:100%;",
                    polyline { points: "{points}", stroke: "{stroke}", stroke_width: "1.5", fill: "none" }
                }
            }
            div { style: "font-family:var(--font-mono); font-size:11px; color:var(--text-secondary); margin-top:6px;", "{caption}" }
        }
    }
}
