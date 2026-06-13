//! Graph Explorer: a top bar with a Cypher / Builder / GraphRAG mode switch
//! over a shared canvas. The mode is local component state.

use dioxus::prelude::*;

use crate::views::graph_explorer::builder::GxBuilder;
use crate::views::graph_explorer::cypher::GxCypher;
use crate::views::graph_explorer::graphrag::GxGraphrag;

#[component]
pub fn GraphExplorer() -> Element {
    let mut mode = use_signal(|| "cypher".to_string());
    let m = mode.read().clone();

    rsx! {
        div { class: "view active",
            div { class: "gx-frame",
                div { class: "gx-top",
                    div { class: "segmented",
                        button { class: if m == "cypher" { "active" } else { "" }, onclick: move |_| mode.set("cypher".to_string()), "Cypher" }
                        button { class: if m == "builder" { "active" } else { "" }, onclick: move |_| mode.set("builder".to_string()), "Builder" }
                        button { class: if m == "graphrag" { "active" } else { "" }, onclick: move |_| mode.set("graphrag".to_string()), "GraphRAG" }
                    }
                    div { style: "font-family: var(--font-mono); font-size: 11px; color: var(--text-tertiary);",
                        "graph: social_graph · 3.2M nodes · 18.4M edges"
                    }
                    div { style: "margin-left: auto; display: flex; gap: 6px;",
                        button { class: "btn small", "Layout: force" }
                        button { class: "btn small", "Limit: 50" }
                        button { class: "btn small primary", "▸ Run" }
                    }
                }
                match m.as_str() {
                    "builder" => rsx! { GxBuilder {} },
                    "graphrag" => rsx! { GxGraphrag {} },
                    _ => rsx! { GxCypher {} },
                }
            }
        }
    }
}
