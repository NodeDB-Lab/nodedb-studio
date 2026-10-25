//! The three GraphRAG result panels: the evidence list, the graph around it,
//! and the chosen record's citation.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::format::date;
use crate::models::graph::{GraphNode, NodeKind, value_text};
use crate::models::graph_layout::LayoutKind;
use crate::models::graph_rag::{Evidence, RagResult};
use crate::state::graph::{CanvasState, Pick, RagUi};
use crate::views::graph::canvas::GraphCanvas;
use crate::views::graph::glyphs::LegendShape;

/// The evidence the panels show: the chosen rank, else the first.
pub fn chosen(result: &RagResult, selected: Option<u32>) -> Option<&Evidence> {
    selected
        .and_then(|r| result.evidence.iter().find(|e| e.rank == r))
        .or_else(|| result.evidence.first())
}

#[component]
pub fn EvidenceBody(
    result: RagResult,
    selected: Option<u32>,
    on_select: EventHandler<u32>,
) -> Element {
    let current = chosen(&result, selected).map(|e| e.rank);
    rsx! {
        ul { class: "evidence-list",
            for e in result.evidence.iter() {
                {
                    let rank = e.rank;
                    let on = current == Some(rank);
                    rsx! {
                        li { key: "{e.record}",
                            button {
                                class: if on { "evidence-row selected" } else { "evidence-row" },
                                "aria-selected": "{on}",
                                onclick: move |_| on_select.call(rank),
                                span { class: "evidence-rank", "{e.rank}" }
                                span { class: "evidence-main",
                                    span { class: "evidence-title", "{e.title}" }
                                    span { class: "evidence-id mono", "{e.id()}" }
                                }
                                span { class: "evidence-score",
                                    span { class: "score-value", "{e.score:.2}" }
                                    span { class: "score-label", "score" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn ContextBody(result: RagResult, ui: Signal<RagUi>, canvas: Signal<CanvasState>) -> Element {
    let selected = ui.read().selected;
    let current = chosen(&result, selected).cloned();
    if result.graph.nodes.is_empty() {
        return rsx! {
            div { class: "empty-state",
                Ico { icon: Icon::Network, size: 28 }
                h3 { "No graph context" }
                p { "The evidence links to no node in the chosen binding, so expansion found nothing to follow." }
            }
        };
    }
    let pick = current
        .as_ref()
        .filter(|e| result.graph.node(&e.record).is_some())
        .map(|e| Pick::Node(e.record.clone()));
    let highlight = current.as_ref().and_then(|e| e.path.clone());
    let evidence = result.evidence.clone();
    rsx! {
        div { class: "graph-region",
            GraphCanvas {
                graph: result.graph.clone(),
                layout_kind: LayoutKind::Force,
                canvas,
                selected: pick,
                highlight,
                on_pick: move |p: Pick| {
                    if let Pick::Node(path) = p
                        && let Some(e) = evidence.iter().find(|e| e.record == path)
                    {
                        ui.write().selected = Some(e.rank);
                    }
                },
            }
        }
    }
}

/// What a node is, in the words a person uses for it.
fn caption(node: &GraphNode) -> String {
    match node.kind {
        NodeKind::User => "Customer".to_string(),
        NodeKind::Order => {
            let when =
                node.property("created_at")
                    .map(|v| match v {
                        nodedb_types::Value::DateTime(t)
                        | nodedb_types::Value::NaiveDateTime(t) => date(t.unix_secs(), 0),
                        other => value_text(other),
                    })
                    .unwrap_or_default();
            format!("Order · {when}")
        }
        NodeKind::Product => node.name.clone().unwrap_or_else(|| node.id().to_string()),
        NodeKind::Post | NodeKind::Topic | NodeKind::Article => node.kind.label().to_string(),
    }
}

fn metric_word(metric: &str) -> String {
    let mut c = metric.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

#[component]
pub fn SelectedBody(result: RagResult, selected: Option<u32>, provider: Option<String>) -> Element {
    let Some(e) = chosen(&result, selected).cloned() else {
        return rsx! {};
    };
    let metric = metric_word(&e.provenance.metric);
    let hops = e.path.as_ref().map_or(0, |p| p.hops());
    rsx! {
        div { class: "inspector evidence-detail",
            div { class: "inspector-head",
                span { class: "evidence-icon", Ico { icon: Icon::FileText, size: 18 } }
                div { class: "evidence-head-text",
                    h3 { class: "inspector-title", "{e.title}" }
                    span { class: "mono muted", "{e.record}" }
                }
            }
            div { class: "kv",
                div { class: "kv-row", span { "Source" } span { class: "mono", "{e.record}" } }
                div { class: "kv-row", span { "Title" } span { "{e.title}" } }
                div { class: "kv-row", span { "{metric} similarity" } span { class: "mono", "{e.score:.2}" } }
                div { class: "kv-row", span { "{metric} distance" } span { class: "mono", "{e.distance():.2}" } }
            }
            div { class: "inspector-section", "Content excerpt" }
            pre { class: "code-block excerpt", "{e.excerpt}" }
            div { class: "inspector-section", "Provenance" }
            ul { class: "provenance",
                li { "Matched by vector index " span { class: "mono", "{e.provenance.index}" } " ({e.provenance.metric})." }
                if e.provenance.seeds.is_empty() {
                    li { "Links to no node in {e.provenance.binding}, so expansion had nothing to follow." }
                } else {
                    li {
                        "Expanded {e.provenance.hops} hops on {e.provenance.binding} from "
                        span { class: "mono", "{e.provenance.seeds.join(\", \")}" }
                        "."
                    }
                }
            }
            if let Some(p) = e.path.clone() {
                div { class: "inspector-section", "Graph path ({hops} hops)" }
                ol { class: "evidence-path",
                    for (i, n) in p.nodes.iter().enumerate() {
                        {
                            let node = result.graph.node(n).cloned();
                            rsx! {
                                li { key: "{n}",
                                    span { class: "path-index", "{i + 1}" }
                                    if let Some(node) = node {
                                        LegendShape { kind: node.kind }
                                        span { class: "path-text",
                                            span { class: "mono", "{node.collection()} / {node.id()}" }
                                            span { class: "muted", "{caption(&node)}" }
                                        }
                                    } else {
                                        span { class: "mono", "{n}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "answer-provider",
                span { class: "provider-label",
                    "Answer provider"
                    span { class: "info-tip", title: "An answer is written by a configured model provider, never by retrieval.",
                        Ico { icon: Icon::Info, size: 14 }
                    }
                }
                span { class: "muted", if let Some(p) = provider.as_ref() { "{p}" } else { "Not configured" } }
                button {
                    class: "link-btn",
                    disabled: true,
                    title: "Provider setup is not designed yet",
                    "Configure provider"
                }
            }
        }
    }
}
