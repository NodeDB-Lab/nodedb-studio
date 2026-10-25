//! The GraphRAG query form: the question, then every part of the retrieval
//! source, then Search. Nothing is implied: the index, binding, model, and
//! dimension are all named.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::graph_rag::{MAX_HOPS, RagSetup};
use crate::state::graph::RagUi;

#[component]
pub fn RagQueryForm(
    ui: Signal<RagUi>,
    setup: RagSetup,
    /// Why Search is off. Empty when it can run.
    issues: Vec<String>,
    on_search: EventHandler<()>,
) -> Element {
    let form = ui.read().form.clone();
    let source = form.source(&setup).cloned();
    let request = form.request(&setup);
    let model = request
        .as_ref()
        .map(|r| r.model.clone())
        .unwrap_or_default();
    let binding = request
        .as_ref()
        .map(|r| r.binding.clone())
        .unwrap_or_default();
    let dimension = request.as_ref().map_or(0, |r| r.dimension);
    let ready = issues.is_empty();
    let first_issue = issues.first().cloned().unwrap_or_default();
    let nodes_invalid = issues.iter().any(|i| i.starts_with("Max nodes"));
    let question_invalid = issues.iter().any(|i| i.starts_with("Enter a question"));

    rsx! {
        form {
            class: "rag-form",
            onsubmit: move |e| {
                e.prevent_default();
                if ready { on_search.call(()); }
            },
            div { class: "field",
                label { r#for: "rag-question", class: "sr-only", "Question" }
                input {
                    id: "rag-question",
                    class: if question_invalid { "input invalid" } else { "input" },
                    placeholder: "Ask a question about your data",
                    value: "{form.question}",
                    "aria-invalid": "{question_invalid}",
                    oninput: move |e| ui.write().form.question = e.value(),
                }
            }
            div { class: "rag-fields",
                div { class: "field",
                    label { r#for: "rag-source", "Retrieval source" }
                    select {
                        id: "rag-source",
                        class: "select native",
                        onchange: move |e| ui.write().form.source = Some(e.value()),
                        for s in setup.sources.iter() {
                            option {
                                key: "{s.label()}",
                                value: "{s.label()}",
                                selected: source.as_ref().is_some_and(|c| c.label() == s.label()),
                                "{s.label()}"
                            }
                        }
                    }
                }
                div { class: "field",
                    label { r#for: "rag-index", "Index" }
                    select { id: "rag-index", class: "select native", disabled: true,
                        option { selected: true,
                            "{source.as_ref().map(|s| s.index.clone()).unwrap_or_default()}"
                        }
                    }
                }
                div { class: "field",
                    label { r#for: "rag-binding", "Graph binding" }
                    select {
                        id: "rag-binding",
                        class: "select native",
                        onchange: move |e| ui.write().form.binding = Some(e.value()),
                        for b in setup.bindings.iter() {
                            option { key: "{b}", value: "{b}", selected: *b == binding, "{b}" }
                        }
                    }
                }
                div { class: "field",
                    label { r#for: "rag-model", "Embedding model" }
                    select {
                        id: "rag-model",
                        class: "select native",
                        onchange: move |e| ui.write().form.model = Some(e.value()),
                        for m in setup.models.iter() {
                            option { key: "{m.name}", value: "{m.name}", selected: m.name == model, "{m.name}" }
                        }
                    }
                }
                div { class: "field narrow",
                    label { r#for: "rag-dimension", "Dimension" }
                    input {
                        id: "rag-dimension",
                        class: "input mono",
                        readonly: true,
                        title: "The embedding model's output size",
                        value: "{dimension}",
                    }
                }
                div { class: "field",
                    label { r#for: "rag-hops", "Graph expansion" }
                    select {
                        id: "rag-hops",
                        class: "select native",
                        onchange: move |e| {
                            if let Ok(n) = e.value().parse::<u8>() { ui.write().form.hops = n; }
                        },
                        for n in 1..=MAX_HOPS {
                            option {
                                key: "{n}",
                                value: "{n}",
                                selected: n == form.hops,
                                if n == 1 { "1 hop" } else { "{n} hops" }
                            }
                        }
                    }
                }
                div { class: "field narrow",
                    label { r#for: "rag-nodes", "Max nodes" }
                    input {
                        id: "rag-nodes",
                        class: if nodes_invalid { "input invalid" } else { "input" },
                        inputmode: "numeric",
                        value: "{form.max_nodes}",
                        "aria-invalid": "{nodes_invalid}",
                        oninput: move |e| ui.write().form.max_nodes = e.value(),
                    }
                }
                button {
                    class: "btn primary rag-search",
                    r#type: "submit",
                    disabled: !ready,
                    title: if ready { "Search the index and expand the graph" } else { "{first_issue}" },
                    Ico { icon: Icon::Search, size: 16 }
                    "Search"
                }
            }
            if !ready {
                div { class: "error", role: "alert",
                    for i in issues.iter() {
                        span { key: "{i}", "{i} " }
                    }
                }
            }
        }
    }
}
