//! FTS inspector: analyzer chain visualization + query tester + index stats.

use dioxus::prelude::*;

#[component]
pub fn FtsInspector() -> Element {
    rsx! {
        div { class: "view active",
            div { class: "view-padded",
                div { class: "view-header",
                    div {
                        h1 { "FTS inspector" }
                        p { "Watch text flow through the analyzer chain. Test queries against the live index." }
                    }
                    div { class: "view-header-actions",
                        button { class: "btn", "Reindex" }
                        button { class: "btn primary", "Save analyzer" }
                    }
                }

                div { class: "card", style: "padding:18px; margin-bottom:14px;",
                    div { class: "eyebrow", style: "margin-bottom:10px;", "Analyzer chain · articles_idx" }
                    div { style: "display:flex; align-items:center; gap:8px; overflow-x:auto;",
                        ChainBox { eyebrow: "Input", lines: vec!["\"The Distributed Graph Database\"".to_string()] }
                        Arrow {}
                        ChainBox { eyebrow: "1 · Tokenizer", lines: vec!["standard".to_string(), "[The, Distributed, Graph, Database]".to_string()] }
                        Arrow {}
                        ChainBox { eyebrow: "2 · Lowercase", lines: vec!["[the, distributed, graph, database]".to_string()] }
                        Arrow {}
                        ChainBox { eyebrow: "3 · Stop words", lines: vec!["[distributed, graph, database]".to_string()] }
                        Arrow {}
                        ChainBox { eyebrow: "4 · Stemmer", lines: vec!["en_porter2".to_string(), "[distribut, graph, databas]".to_string()] }
                        Arrow {}
                        div { style: "background:var(--bg-primary); border:1px solid var(--text-success); border-radius:6px; padding:10px 14px; min-width:140px;",
                            div { class: "eyebrow", style: "color:var(--text-success)", "Output tokens" }
                            div { style: "font-family:var(--font-mono); font-size:11px; margin-top:4px;", "distribut graph databas" }
                        }
                    }
                    div { style: "margin-top:12px; display:flex; gap:6px;",
                        button { class: "btn small", "+ Filter" }
                        button { class: "btn small", "Reorder" }
                        button { class: "btn small ghost", "Reset to default" }
                    }
                }

                div { style: "display:grid; grid-template-columns: 1fr 1fr; gap: 14px;",
                    div { class: "card", style: "padding:18px;",
                        div { class: "eyebrow", style: "margin-bottom:10px;", "Query tester" }
                        input { value: "distributed graph databases", style: "width:100%; padding:8px 12px; background:var(--bg-secondary); border:0.5px solid var(--border-mid); border-radius:5px; font-family:var(--font-mono); font-size:12px;" }
                        div { style: "margin-top:14px; display:grid; grid-template-columns: auto 1fr; gap:6px 12px; font-family:var(--font-mono); font-size:11px;",
                            span { style: "color:var(--text-tertiary)", "analyzed" } span { "[distribut, graph, databas]" }
                            span { style: "color:var(--text-tertiary)", "hits" } span { "241" }
                            span { style: "color:var(--text-tertiary)", "took" } span { "14ms" }
                            span { style: "color:var(--text-tertiary)", "max score" } span { "0.94" }
                        }
                        div { style: "margin-top:14px;",
                            div { class: "eyebrow", style: "margin-bottom:6px;", "Top results" }
                            div { style: "font-family:var(--font-mono); font-size:11px; line-height:1.8;",
                                div { span { style: "color:var(--text-warning)", "0.94" } " · Designing a multi-engine database" }
                                // De-arcadedb'd (mockup said "Why ArcadeDB chose multi-model").
                                div { span { style: "color:var(--text-warning)", "0.88" } " · Why NodeDB chose multi-model" }
                                div { span { style: "color:var(--text-warning)", "0.71" } " · NodeDB roadmap to 1.0" }
                            }
                        }
                    }
                    div { class: "card", style: "padding:18px;",
                        div { class: "eyebrow", style: "margin-bottom:10px;", "Index stats" }
                        div { style: "display:grid; grid-template-columns:1fr 1fr; gap: 10px 16px; font-family:var(--font-mono); font-size:11px;",
                            Stat { label: "documents", value: "241,005" }
                            Stat { label: "unique terms", value: "88,402" }
                            Stat { label: "avg doc length", value: "412 tokens" }
                            Stat { label: "index size", value: "182 MB" }
                            Stat { label: "last update", value: "2s ago" }
                            Stat { label: "segments", value: "14" }
                        }
                        div { style: "margin-top:18px;",
                            div { class: "eyebrow", style: "margin-bottom:6px;", "Most frequent terms" }
                            div { style: "font-family:var(--font-mono); font-size:11px; line-height:1.8;",
                                TermBar { term: "databas", width: "120px", count: "18,402" }
                                TermBar { term: "graph", width: "104px", count: "15,108" }
                                TermBar { term: "queri", width: "88px", count: "12,890" }
                                TermBar { term: "distribut", width: "64px", count: "8,201" }
                                TermBar { term: "vector", width: "52px", count: "6,841" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn Arrow() -> Element {
    rsx! { span { style: "color:var(--text-tertiary); font-family:var(--font-mono);", "→" } }
}

#[component]
fn ChainBox(eyebrow: String, lines: Vec<String>) -> Element {
    rsx! {
        div { style: "background:var(--bg-secondary); border:0.5px solid var(--border-mid); border-radius:6px; padding:10px 14px; min-width:140px;",
            div { class: "eyebrow", "{eyebrow}" }
            for line in lines {
                div { style: "font-family:var(--font-mono); font-size:10px; color:var(--text-tertiary); margin-top:4px;", "{line}" }
            }
        }
    }
}

#[component]
fn Stat(label: String, value: String) -> Element {
    rsx! {
        div {
            div { style: "color:var(--text-tertiary)", "{label}" }
            div { style: "font-size:14px; margin-top:2px;", "{value}" }
        }
    }
}

#[component]
fn TermBar(term: String, width: String, count: String) -> Element {
    rsx! {
        div {
            span { style: "display:inline-block; width:60px;", "{term}" }
            " "
            span { style: "display:inline-block; width:{width}; height:6px; background:#185fa5; opacity:0.7; vertical-align:middle;" }
            " {count}"
        }
    }
}
