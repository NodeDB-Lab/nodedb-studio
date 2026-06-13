//! FTS storage-mode viewer: a search box + ranked results with highlights.

use dioxus::prelude::*;

#[component]
pub fn FtsViewer() -> Element {
    rsx! {
        div { class: "engine-viewer",
            div { style: "padding: 16px 20px; border-bottom: 0.5px solid var(--border-soft);",
                input {
                    style: "width:100%; padding: 8px 12px; background: var(--bg-secondary); border: 0.5px solid var(--border-mid); border-radius: 5px; font-family: var(--font-mono); font-size: 12px;",
                    placeholder: "Search 'articles_idx' — try: distributed graph database",
                }
            }
            div { class: "data-grid-wrap",
                table { class: "data-grid",
                    thead { tr { th { "score" } th { "id" } th { "title" } th { "snippet" } } }
                    tbody {
                        tr {
                            td { "0.94" }
                            td { "art_2841" }
                            td { "Designing a multi-engine database" }
                            td { "\"…" b { "distributed" } " systems with a " b { "graph" } " layer on top of…\"" }
                        }
                        tr {
                            td { "0.88" }
                            td { "art_1820" }
                            // Mockup said "Why ArcadeDB chose multi-model"; de-arcadedb'd.
                            td { "Why NodeDB chose multi-model" }
                            td { "\"…native " b { "graph" } " traversals over a " b { "distributed" } " KV store…\"" }
                        }
                        tr {
                            td { "0.71" }
                            td { "art_3018" }
                            td { "NodeDB roadmap to 1.0" }
                            td { "\"…a " b { "distributed" } " CRDT layer and a " b { "graph" } " projection…\"" }
                        }
                    }
                }
            }
        }
    }
}
