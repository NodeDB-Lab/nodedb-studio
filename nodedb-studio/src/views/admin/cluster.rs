//! Admin · Cluster topology. Placeholder SVG node-link diagram (no real graph
//! renderer). Node names de-arcadedb'd to node-N; version shown as "dev".

use dioxus::prelude::*;

#[component]
pub fn ClusterView() -> Element {
    rsx! {
        div { class: "subview full active",
            div { class: "cluster-canvas",
                svg { view_box: "0 0 1000 600", style: "width:100%; height:100%;",
                    circle { cx: "500", cy: "300", r: "180", fill: "none", stroke: "rgba(24,95,165,0.15)", stroke_width: "60" }
                    circle { cx: "500", cy: "300", r: "260", fill: "none", stroke: "rgba(59,109,17,0.10)", stroke_width: "40" }
                    g { stroke: "var(--border-strong)", stroke_width: "0.5", fill: "none",
                        path { d: "M500,300 L320,200" }
                        path { d: "M500,300 L680,200" }
                        path { d: "M500,300 L320,400" }
                        path { d: "M500,300 L680,400" }
                        path { d: "M320,200 L680,200" }
                        path { d: "M320,400 L680,400" }
                        path { d: "M320,200 L320,400" }
                        path { d: "M680,200 L680,400" }
                        path { d: "M500,300 L500,120" }
                        path { d: "M500,300 L500,480" }
                    }
                    g {
                        circle { class: "node-circle node-leader", cx: "500", cy: "300", r: "32" }
                        text { x: "500", y: "304", text_anchor: "middle", fill: "white", font_family: "var(--font-mono)", font_size: "10", font_weight: "600", "L1" }
                        text { x: "500", y: "350", text_anchor: "middle", fill: "var(--text-primary)", font_family: "var(--font-mono)", font_size: "10", "node-1 · leader" }
                    }
                    g {
                        circle { class: "node-circle node-follower", cx: "320", cy: "200", r: "24" }
                        text { x: "320", y: "204", text_anchor: "middle", fill: "white", font_family: "var(--font-mono)", font_size: "9", "F" }
                        text { x: "320", y: "240", text_anchor: "middle", font_family: "var(--font-mono)", font_size: "10", fill: "var(--text-primary)", "node-2" }
                    }
                    g {
                        circle { class: "node-circle node-follower", cx: "680", cy: "200", r: "24" }
                        text { x: "680", y: "204", text_anchor: "middle", fill: "white", font_family: "var(--font-mono)", font_size: "9", "F" }
                        text { x: "680", y: "240", text_anchor: "middle", font_family: "var(--font-mono)", font_size: "10", fill: "var(--text-primary)", "node-3" }
                    }
                    g {
                        circle { class: "node-circle node-follower", cx: "320", cy: "400", r: "24" }
                        text { x: "320", y: "404", text_anchor: "middle", fill: "white", font_family: "var(--font-mono)", font_size: "9", "F" }
                        text { x: "320", y: "440", text_anchor: "middle", font_family: "var(--font-mono)", font_size: "10", fill: "var(--text-primary)", "node-4" }
                    }
                    g {
                        circle { class: "node-circle node-degraded", cx: "680", cy: "400", r: "24" }
                        text { x: "680", y: "404", text_anchor: "middle", fill: "white", font_family: "var(--font-mono)", font_size: "9", "!" }
                        text { x: "680", y: "440", text_anchor: "middle", font_family: "var(--font-mono)", font_size: "10", fill: "var(--text-primary)", "node-5 · lag" }
                    }
                    g {
                        circle { class: "node-circle node-follower", cx: "500", cy: "120", r: "24" }
                        text { x: "500", y: "124", text_anchor: "middle", fill: "white", font_family: "var(--font-mono)", font_size: "9", "F" }
                        text { x: "500", y: "160", text_anchor: "middle", font_family: "var(--font-mono)", font_size: "10", fill: "var(--text-primary)", "node-6" }
                    }
                    g {
                        circle { class: "node-circle node-follower", cx: "500", cy: "480", r: "24" }
                        text { x: "500", y: "484", text_anchor: "middle", fill: "white", font_family: "var(--font-mono)", font_size: "9", "F" }
                        text { x: "500", y: "520", text_anchor: "middle", font_family: "var(--font-mono)", font_size: "10", fill: "var(--text-primary)", "node-7" }
                    }
                    text { x: "500", y: "50", text_anchor: "middle", font_family: "var(--font-mono)", font_size: "11", fill: "var(--text-tertiary)", "staging-cluster · raft-group-1 · term 142" }
                }
                div { style: "position:absolute; top:14px; right:14px; padding: 12px 14px; background: var(--bg-primary); border: 0.5px solid var(--border-mid); border-radius: 8px; font-size: 11px; min-width: 200px;",
                    div { class: "eyebrow", style: "margin-bottom: 6px;", "Cluster" }
                    div { style: "display:grid; grid-template-columns: auto 1fr; gap: 4px 12px; font-family: var(--font-mono);",
                        span { style: "color:var(--text-tertiary)", "nodes" }
                        span { "6 / 7 healthy" }
                        span { style: "color:var(--text-tertiary)", "leader" }
                        span { "node-1" }
                        span { style: "color:var(--text-tertiary)", "term" }
                        span { "142" }
                        span { style: "color:var(--text-tertiary)", "commit idx" }
                        span { "1,488,201" }
                        span { style: "color:var(--text-tertiary)", "version" }
                        span { "dev" }
                    }
                }
            }
        }
    }
}
