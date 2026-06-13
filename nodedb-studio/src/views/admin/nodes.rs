//! Admin · Per-node throughput: small-multiple sparklines per node.
//! Placeholder SVG sparklines (no real chart library).

use dioxus::prelude::*;

struct Node {
    name: &'static str,
    pill_css: &'static str,
    pill_text: &'static str,
    points: &'static str,
    stroke: &'static str,
    qps: &'static str,
    p99: &'static str,
    p99_warn: bool,
    cpu: &'static str,
}

#[component]
pub fn NodesView() -> Element {
    let nodes = [
        Node {
            name: "node-1 · leader",
            pill_css: "ok",
            pill_text: "ok",
            points: "0,60 40,55 80,50 120,45 160,40 200,42 240,30 280,32 320,28 360,18 400,22",
            stroke: "#185fa5",
            qps: "2.1k",
            p99: "18ms",
            p99_warn: false,
            cpu: "42%",
        },
        Node {
            name: "node-2",
            pill_css: "ok",
            pill_text: "ok",
            points: "0,50 40,52 80,48 120,55 160,45 200,40 240,38 280,42 320,32 360,28 400,30",
            stroke: "#3b6d11",
            qps: "1.8k",
            p99: "22ms",
            p99_warn: false,
            cpu: "38%",
        },
        Node {
            name: "node-3",
            pill_css: "ok",
            pill_text: "ok",
            points: "0,45 40,48 80,42 120,50 160,38 200,35 240,40 280,32 320,28 360,30 400,25",
            stroke: "#3b6d11",
            qps: "1.9k",
            p99: "19ms",
            p99_warn: false,
            cpu: "41%",
        },
        Node {
            name: "node-4",
            pill_css: "ok",
            pill_text: "ok",
            points: "0,55 40,48 80,52 120,42 160,45 200,38 240,35 280,40 320,30 360,32 400,28",
            stroke: "#3b6d11",
            qps: "1.6k",
            p99: "24ms",
            p99_warn: false,
            cpu: "36%",
        },
        Node {
            name: "node-5",
            pill_css: "warn",
            pill_text: "lag",
            points: "0,30 40,40 80,35 120,55 160,50 200,62 240,58 280,68 320,65 360,72 400,70",
            stroke: "#854f0b",
            qps: "980",
            p99: "182ms",
            p99_warn: true,
            cpu: "78%",
        },
        Node {
            name: "node-6",
            pill_css: "ok",
            pill_text: "ok",
            points: "0,50 40,45 80,48 120,40 160,42 200,35 240,38 280,30 320,32 360,25 400,28",
            stroke: "#3b6d11",
            qps: "1.7k",
            p99: "21ms",
            p99_warn: false,
            cpu: "39%",
        },
    ];
    rsx! {
        div { class: "subview active",
            div { class: "view-header",
                div { h1 { style: "font-size:16px;", "Per-node throughput" } p { "Small multiples · last 1 hour" } }
                div { class: "view-header-actions",
                    div { class: "segmented",
                        button { "15m" }
                        button { class: "active", "1h" }
                        button { "6h" }
                        button { "24h" }
                    }
                }
            }
            div { style: "display:grid; grid-template-columns:repeat(auto-fit, minmax(280px, 1fr)); gap: 12px;",
                for n in nodes {
                    div { class: "card", style: "padding:14px 16px;",
                        div { style: "display:flex; align-items:center; justify-content:space-between;",
                            strong { "{n.name}" }
                            span { class: "pill {n.pill_css}", span { class: "dot" } "{n.pill_text}" }
                        }
                        div { style: "margin-top:8px; height:80px;",
                            svg { view_box: "0 0 400 80", preserve_aspect_ratio: "none", style: "width:100%; height:100%;",
                                polyline { points: "{n.points}", stroke: "{n.stroke}", stroke_width: "1.5", fill: "none" }
                            }
                        }
                        div { style: "display:flex; gap:12px; margin-top:8px; font-family:var(--font-mono); font-size:11px;",
                            span { span { style: "color:var(--text-tertiary)", "qps " } "{n.qps}" }
                            span {
                                span { style: "color:var(--text-tertiary)", "p99 " }
                                if n.p99_warn {
                                    span { style: "color:var(--text-warning)", "{n.p99}" }
                                } else {
                                    "{n.p99}"
                                }
                            }
                            span { span { style: "color:var(--text-tertiary)", "cpu " } "{n.cpu}" }
                        }
                    }
                }
            }
        }
    }
}
