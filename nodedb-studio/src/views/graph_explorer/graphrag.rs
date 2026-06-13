//! Graph Explorer · GraphRAG fusion: ranks vector neighbors alongside graph
//! traversal, with provenance.

use dioxus::prelude::*;

#[component]
pub fn GxGraphrag() -> Element {
    rsx! {
        div { class: "gx-body",
            div { class: "gx-editor", style: "height: 120px;",
                pre {
                    span { class: "sql-comment", "-- ranks vector neighbors alongside graph traversal, with provenance" }
                    "\n"
                    span { class: "cypher-kw", "FUSE" }
                    " vec("
                    span { class: "cypher-str", "\"databases that combine vectors and graphs\"" }
                    ", k=10)\n   "
                    span { class: "cypher-kw", "WITH" }
                    " graph((u:User {{id: "
                    span { class: "cypher-str", "\"u_44182\"" }
                    "}})-[*1..2]->(d:Doc))\n   "
                    span { class: "cypher-kw", "RETURN" }
                    " d, score, provenance "
                    span { class: "cypher-kw", "ORDER BY" }
                    " score "
                    span { class: "cypher-kw", "DESC" }
                }
            }
            div { class: "grag-split",
                div { class: "grag-results",
                    div { class: "grag-result",
                        div { class: "grag-result-head",
                            span { class: "grag-score vec", "vec 0.92" }
                            span { class: "grag-score graph", "graph 0.81" }
                            strong { "Designing a multi-engine database" }
                        }
                        div { class: "grag-result-body", "\"…blending HNSW vector search with native graph traversals over a shared storage layer is the next decade's design problem…\"" }
                        div { class: "grag-prov", "via: u_44182 → POSTED → p_8281 → CITES → d_2841" }
                    }
                    div { class: "grag-result",
                        div { class: "grag-result-head",
                            span { class: "grag-score vec", "vec 0.88" }
                            span { class: "grag-score graph", "graph 0.74" }
                            // De-arcadedb'd (mockup said "Why ArcadeDB chose multi-model").
                            strong { "Why NodeDB chose multi-model" }
                        }
                        div { class: "grag-result-body", "\"…we wanted graph queries to be a first-class projection over the same physical storage as documents, KV, and time-series…\"" }
                        div { class: "grag-prov", "via: u_44182 → FOLLOWS → u_22018 → POSTED → d_1820" }
                    }
                    div { class: "grag-result",
                        div { class: "grag-result-head",
                            span { class: "grag-score vec", "vec 0.77" }
                            span { class: "grag-score graph", "graph 0.69" }
                            strong { "NodeDB roadmap to 1.0" }
                        }
                        div { class: "grag-result-body", "\"…the GraphRAG fusion view in Studio collapses two retrieval methods into a single ranked list with clear provenance…\"" }
                        div { class: "grag-prov", "via: u_44182 → BOOKMARKED → d_3018" }
                    }
                }
                div { style: "background: var(--bg-secondary); border-left: 0.5px solid var(--border-mid); padding: 16px;",
                    div { class: "eyebrow", style: "margin-bottom: 10px;", "Fusion weights" }
                    div { class: "form-field", label { "Vector weight" } input { r#type: "range", min: "0", max: "100", value: "60" } }
                    div { class: "form-field", label { "Graph weight" } input { r#type: "range", min: "0", max: "100", value: "40" } }
                    div { class: "form-field", label { "Provenance hops" } select { option { "≤ 2" } option { "≤ 3" } } }
                    div { class: "eyebrow", style: "margin-top: 18px; margin-bottom: 8px;", "Why this works" }
                    p { style: "font-size: 11px; color: var(--text-secondary); line-height: 1.6;",
                        "Vector search finds semantically near documents; graph traversal finds documents your network has interacted with. Fusion ranks documents that are "
                        em { "both" }
                        " meaningful and connected."
                    }
                }
            }
        }
    }
}
