//! Vector storage-mode viewer: index metadata + sample rows.

use dioxus::prelude::*;

#[component]
pub fn VectorViewer() -> Element {
    rsx! {
        div { class: "engine-viewer",
            div { class: "vector-meta",
                div { class: "m", div { class: "lbl", "Dimensions" } div { class: "val", "768" } div { class: "sub", "all-mpnet-base-v2" } }
                div { class: "m", div { class: "lbl", "Index" } div { class: "val", "HNSW" } div { class: "sub", "M=16, ef=200" } }
                div { class: "m", div { class: "lbl", "Vectors" } div { class: "val", "1,108,492" } div { class: "sub", "+412 / hr" } }
                div { class: "m", div { class: "lbl", "Distance" } div { class: "val", "cosine" } div { class: "sub", "recall@10 = 0.94" } }
            }
            div { style: "padding: 14px 20px;",
                div { class: "eyebrow", style: "margin-bottom: 8px;", "Sample rows" }
                div { class: "data-grid-wrap",
                    table { class: "data-grid",
                        thead { tr { th { "id" } th { "source_doc" } th { "chunk" } th { "vector preview" } } }
                        tbody {
                            tr {
                                td { "e_001" } td { "handbook.md#42" }
                                td { "\"Engine-aware viewers open a purpose-built UI…\"" }
                                td { "[0.041, -0.218, 0.110, …]" }
                            }
                            tr {
                                td { "e_002" } td { "handbook.md#43" }
                                td { "\"Streams & Events is treated as a primary…\"" }
                                td { "[-0.089, 0.412, 0.022, …]" }
                            }
                            tr {
                                td { "e_003" } td { "handbook.md#44" }
                                td { "\"The GraphRAG fusion view ranks vector neighbors…\"" }
                                td { "[0.155, 0.001, -0.244, …]" }
                            }
                        }
                    }
                }
            }
        }
    }
}
