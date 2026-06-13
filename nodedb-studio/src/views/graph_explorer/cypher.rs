//! Graph Explorer · Cypher mode: query editor + canvas + selection drawer.
//! Canvas is a placeholder SVG (no real graph renderer).

use dioxus::prelude::*;

#[component]
pub fn GxCypher() -> Element {
    rsx! {
        div { class: "gx-body",
            div { class: "gx-editor",
                pre {
                    span { class: "cypher-kw", "MATCH" }
                    " (u:User)-[:POSTED]->(p:Post)-[:TAGGED]->(t:Topic {{name: "
                    span { class: "cypher-str", "\"databases\"" }
                    "}})\n"
                    span { class: "cypher-kw", "WHERE" }
                    " u.country = "
                    span { class: "cypher-str", "\"MY\"" }
                    "\n"
                    span { class: "cypher-kw", "RETURN" }
                    " u, p, t\n"
                    span { class: "cypher-kw", "LIMIT" }
                    " 50"
                }
            }
            div { class: "gx-canvas-shell",
                div { class: "gx-canvas",
                    svg { view_box: "0 0 1000 500", style: "width:100%; height:100%;",
                        path { class: "gx-edge", d: "M200,250 Q 280 180 360 200" }
                        path { class: "gx-edge", d: "M200,250 Q 280 320 360 300" }
                        path { class: "gx-edge", d: "M360,200 Q 470 240 580 230" }
                        path { class: "gx-edge", d: "M360,300 Q 470 270 580 280" }
                        path { class: "gx-edge", d: "M580,230 L 720 200" }
                        path { class: "gx-edge", d: "M580,280 L 720 300" }
                        path { class: "gx-edge", d: "M720,200 Q 800 250 850 230" }
                        path { class: "gx-edge", d: "M720,300 Q 800 250 850 270" }
                        g { class: "gx-node", circle { cx: "200", cy: "250", r: "26", fill: "#185fa5" } text { x: "200", y: "254", fill: "white", "User" } }
                        g { class: "gx-node", circle { cx: "360", cy: "200", r: "20", fill: "#3b6d11" } text { x: "360", y: "204", fill: "white", "Post" } }
                        g { class: "gx-node", circle { cx: "360", cy: "300", r: "20", fill: "#3b6d11" } text { x: "360", y: "304", fill: "white", "Post" } }
                        g { class: "gx-node", circle { cx: "580", cy: "230", r: "20", fill: "#3b6d11" } text { x: "580", y: "234", fill: "white", "Post" } }
                        g { class: "gx-node", circle { cx: "580", cy: "280", r: "20", fill: "#3b6d11" } text { x: "580", y: "284", fill: "white", "Post" } }
                        g { class: "gx-node", circle { cx: "720", cy: "200", r: "20", fill: "#185fa5" } text { x: "720", y: "204", fill: "white", "User" } }
                        g { class: "gx-node", circle { cx: "720", cy: "300", r: "20", fill: "#185fa5" } text { x: "720", y: "304", fill: "white", "User" } }
                        g { class: "gx-node", circle { cx: "850", cy: "250", r: "30", fill: "#854f0b" } text { x: "850", y: "254", fill: "white", "Topic" } }
                    }
                }
                div { class: "gx-drawer-col",
                    div { class: "eyebrow", style: "margin-bottom:10px;", "Selection" }
                    h4 { "User · u_44182" }
                    div { class: "kv",
                        div { class: "k", "_id" } div { class: "v", "u_44182" }
                        div { class: "k", "name" } div { class: "v", "Aisha Tan" }
                        div { class: "k", "country" } div { class: "v", "MY" }
                        div { class: "k", "joined" } div { class: "v", "2024-03-11" }
                        div { class: "k", "degree" } div { class: "v", "out: 412 · in: 88" }
                    }
                    div { style: "margin-top: 12px; display: flex; gap: 6px;",
                        button { class: "btn small", "Expand" }
                        button { class: "btn small", "Find paths" }
                    }
                    div { class: "eyebrow", style: "margin-top: 22px; margin-bottom: 8px;", "Path to Topic" }
                    div { style: "font-family: var(--font-mono); font-size: 11px; color: var(--text-secondary); line-height: 1.7;",
                        "User(u_44182)" br {}
                        "─[POSTED]→ Post(p_8281)" br {}
                        "─[TAGGED]→ Topic(databases)"
                    }
                }
            }
        }
    }
}
