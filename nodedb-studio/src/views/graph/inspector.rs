//! The selection inspector: the selected node or edge, what Expand and Find
//! paths do with it, and the path its result draws.
//!
//! One body serves the Cypher tab's Selection panel and the Builder's Node
//! details panel. `Flavor` picks the sections each shows.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::collection::{Attached, Collection};
use crate::models::format::date;
use crate::models::graph::{GraphEdge, GraphNode, GraphPath, GraphResult, value_text};
use crate::routes::Route;
use crate::services::backend::Backend;
use crate::state::explorer::{
    CollectionTab, RecordView, Selected, select_collection, select_record,
};
use crate::state::graph::{CanvasState, EXPAND_LIMIT, Notice, PATH_HOPS, Pick, target_kind};
use crate::views::graph::glyphs::{LegendShape, kind_tone};

/// Which sections the inspector shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flavor {
    /// The Cypher tab: the path to where the query ends.
    Selection,
    /// The Builder tab: actions, Open record, and the node's relations.
    Details,
}

/// The properties a node lists, without the name its title already shows.
fn rows(node: &GraphNode) -> Vec<(String, String)> {
    let mut out = vec![("_id".to_string(), node.id().to_string())];
    for (k, v) in &node.properties {
        if k == "name" && node.name.is_some() {
            continue;
        }
        out.push((k.clone(), value_text(v)));
    }
    out.push((
        "degree".to_string(),
        format!("out: {} · in: {}", node.degree.out, node.degree.inbound),
    ));
    out
}

/// `User(u44182)`.
fn step_label(graph: &GraphResult, path: &str) -> String {
    graph.node(path).map_or_else(
        || path.to_string(),
        |n| format!("{}({})", n.kind.label(), n.id()),
    )
}

fn edge_date(e: &GraphEdge) -> String {
    e.properties
        .iter()
        .find(|(k, _)| k == "created_at")
        .and_then(|(_, v)| match v {
            nodedb_types::Value::DateTime(t) | nodedb_types::Value::NaiveDateTime(t) => {
                Some(date(t.unix_secs(), 0))
            }
            _ => None,
        })
        .unwrap_or_default()
}

#[component]
fn PathSteps(graph: GraphResult, path: GraphPath) -> Element {
    rsx! {
        ol { class: "path-steps",
            for (i, node) in path.nodes.iter().enumerate() {
                {
                    let kind = graph.node(node).map(|n| n.kind);
                    let relation = path
                        .edges
                        .get(i)
                        .and_then(|id| graph.edge(id))
                        .map(|e| e.relation.label());
                    rsx! {
                        li { key: "{node}",
                            span { class: "path-node",
                                if let Some(k) = kind {
                                    LegendShape { kind: k }
                                }
                                "{step_label(&graph, node)}"
                            }
                            if let Some(rel) = relation {
                                span { class: "path-hop",
                                    Ico { icon: Icon::ArrowDown, size: 14 }
                                    span { class: "badge gray", "{rel}" }
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
pub fn GraphInspector(
    /// The run's result, before anything expanded into it.
    result: GraphResult,
    canvas: Signal<CanvasState>,
    binding: String,
    flavor: Flavor,
    /// The Explorer's catalog, to know which records open there.
    catalog: Vec<Collection>,
) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let nav = use_navigator();
    let mut selected = use_context::<Signal<Option<Selected>>>();
    let mut view = use_context::<Signal<RecordView>>();

    let state = canvas.read().clone();
    let visible = state.visible(&result);
    let Some(pick) = state.effective_pick(&visible) else {
        return rsx! { div { class: "empty-state", p { "Nothing is selected." } } };
    };

    let node: Option<GraphNode> = match &pick {
        Pick::Node(p) => visible.node(p).cloned(),
        Pick::Edge(_) => None,
    };
    let edge: Option<GraphEdge> = match &pick {
        Pick::Edge(id) => visible.edge(id).cloned(),
        Pick::Node(_) => None,
    };

    if let Some(e) = edge {
        let ends = [("From", e.from.clone()), ("To", e.to.clone())];
        return rsx! {
            div { class: "inspector graph-inspector",
                div { class: "inspector-head",
                    span { class: "badge gray", "{e.relation.label()}" }
                    h3 { class: "inspector-title", "Edge" }
                }
                div { class: "kv",
                    div { class: "kv-row", span { "_id" } span { class: "mono", "{e.id}" } }
                    div { class: "kv-row", span { "relation" } span { "{e.relation.label()}" } }
                    for (label, path) in ends {
                        div { key: "{label}", class: "kv-row",
                            span { "{label}" }
                            button {
                                class: "link-btn mono",
                                title: "Select {path}",
                                onclick: move |_| canvas.write().select(Pick::Node(path.clone())),
                                "{path}"
                            }
                        }
                    }
                    for (k, v) in e.properties.iter() {
                        div { key: "{k}", class: "kv-row", span { "{k}" } span { class: "mono", "{value_text(v)}" } }
                    }
                }
            }
        };
    }

    let Some(node) = node else {
        return rsx! {};
    };
    let target = target_kind(&result, &visible, &node.path);
    let paths = state.paths_for(&result, &node.path);
    let shown = paths
        .get(state.path_ix.min(paths.len().saturating_sub(1)))
        .cloned();
    let busy = state.busy;
    let in_catalog = catalog.iter().any(|c| c.name == node.collection());
    let record_title = if in_catalog {
        format!("Open {} in the Explorer", node.path)
    } else {
        format!("{} is not in this database's catalog", node.collection())
    };
    let find_title = match target {
        Some(k) => format!("Find paths to {} in {binding}", k.label()),
        None if visible.edges.is_empty() => "The result has no edges to follow".to_string(),
        None => format!(
            "{} is where the visible paths end. Select a node that starts one.",
            node.kind.label()
        ),
    };

    let expand = {
        let (backend, binding, result, path) = (
            backend.clone(),
            binding.clone(),
            result.clone(),
            node.path.clone(),
        );
        let title = node.title();
        move |_| {
            let (backend, binding, result, path, title) = (
                backend.clone(),
                binding.clone(),
                result.clone(),
                path.clone(),
                title.clone(),
            );
            canvas.write().busy = true;
            spawn_forever(async move {
                let read = backend.expand_node(&binding, &path, EXPAND_LIMIT).await;
                let mut c = canvas.write();
                c.busy = false;
                c.notice = Some(match read {
                    Ok(found) => {
                        let (n, e) = c.absorb(&result, &found);
                        let mut text = if n + e == 0 {
                            format!("No new neighbors for {title}.")
                        } else {
                            format!("Added {n} nodes and {e} edges around {title}.")
                        };
                        if found.truncated {
                            text.push_str(&format!(
                                " More neighbors exist than the {EXPAND_LIMIT} read."
                            ));
                        }
                        Notice { error: false, text }
                    }
                    Err(err) => Notice {
                        error: true,
                        text: format!("Expand failed: {err}"),
                    },
                });
            });
        }
    };

    let find = {
        let (backend, binding, result, path) = (
            backend.clone(),
            binding.clone(),
            result.clone(),
            node.path.clone(),
        );
        move |_| {
            let Some(kind) = target else { return };
            let (backend, binding, result, path) = (
                backend.clone(),
                binding.clone(),
                result.clone(),
                path.clone(),
            );
            canvas.write().busy = true;
            spawn_forever(async move {
                let read = backend.find_paths(&binding, &path, kind, PATH_HOPS).await;
                let mut c = canvas.write();
                c.busy = false;
                c.notice = Some(match read {
                    Ok(set) => {
                        c.absorb(&result, &set.graph);
                        let n = set.paths.len();
                        c.paths = set.paths;
                        c.path_ix = 0;
                        c.searched = true;
                        Notice {
                            error: false,
                            text: if n == 0 {
                                format!(
                                    "No path to {} within {PATH_HOPS} hops in {binding}.",
                                    kind.label()
                                )
                            } else {
                                format!(
                                    "{n} paths to {} within {PATH_HOPS} hops in {binding}.",
                                    kind.label()
                                )
                            },
                        }
                    }
                    Err(err) => Notice {
                        error: true,
                        text: format!("Find paths failed: {err}"),
                    },
                });
            });
        }
    };

    let open_record = {
        let (collection, id) = (node.collection().to_string(), node.id().to_string());
        move |_| {
            select_record(&mut selected, &mut view, &collection, &id);
            nav.push(Route::Explorer {});
        }
    };

    let relations: Vec<(GraphEdge, String, String, String)> = visible
        .edges_of(&node.path)
        .take(5)
        .map(|e| {
            let other = if e.from == node.path { &e.to } else { &e.from };
            let o = visible.node(other);
            let meta = o
                .and_then(|n| n.property("total"))
                .map(|v| format!("${}", value_text(v)))
                .unwrap_or_default();
            (e.clone(), other.clone(), meta, edge_date(e))
        })
        .collect();
    let total_edges = node.degree.out + node.degree.inbound;
    let graph_collection = catalog
        .iter()
        .find(|c| c.name == node.collection() && c.has(Attached::Graph))
        .map(|c| c.name.clone());
    let first_relation = visible
        .edges_of(&node.path)
        .next()
        .map(|e| e.relation.label());
    let open_edges = {
        let collection = node.collection().to_string();
        move |_| {
            select_collection(&mut selected, &mut view, &collection);
            {
                let mut v = view.write();
                v.tab = CollectionTab::Graph;
                v.relation = first_relation.map(str::to_string);
            }
            nav.push(Route::Explorer {});
        }
    };
    let title = node.title();
    let paths_count = paths.len();
    let path_ix = state.path_ix.min(paths_count.saturating_sub(1));

    rsx! {
        div { class: "inspector graph-inspector",
            div { class: "inspector-head",
                LegendShape { kind: node.kind }
                span { class: "{kind_tone(node.kind)}", "{node.kind.label()}" }
                h3 { class: "inspector-title", "{title}" }
            }
            div { class: "kv",
                for (k, v) in rows(&node) {
                    div { key: "{k}", class: "kv-row",
                        span { "{k}" }
                        span { class: "mono", "{v}" }
                    }
                }
            }
            if flavor == Flavor::Details {
                div { class: "inspector-section", "Actions" }
            }
            div { class: "button-pair graph-actions",
                button {
                    class: "btn",
                    disabled: busy,
                    title: "Add this node's neighbors from {binding}",
                    onclick: expand,
                    Ico { icon: Icon::Maximize, size: 16 }
                    "Expand"
                }
                button {
                    class: "btn",
                    disabled: busy || target.is_none(),
                    title: "{find_title}",
                    onclick: find,
                    Ico { icon: Icon::GitFork, size: 16 }
                    "Find paths"
                }
                if flavor == Flavor::Details {
                    button {
                        class: "btn",
                        disabled: !in_catalog,
                        title: "{record_title}",
                        onclick: open_record,
                        Ico { icon: Icon::FileText, size: 16 }
                        "Open record"
                    }
                }
            }
            if let Some(n) = state.notice.clone() {
                div {
                    class: if n.error { "callout err" } else { "callout info" },
                    role: if n.error { "alert" } else { "status" },
                    Ico { icon: if n.error { Icon::CircleAlert } else { Icon::Info }, size: 16 }
                    span { "{n.text}" }
                }
            }
            if flavor == Flavor::Selection || state.searched {
                if let (Some(p), Some(k)) = (shown, target) {
                    div { class: "inspector-section path-title",
                        Ico { icon: Icon::GitFork, size: 16 }
                        "Path to {k.label()}"
                        if paths_count > 1 {
                            span { class: "path-pager",
                                button {
                                    class: "icon-btn sm",
                                    disabled: path_ix == 0,
                                    "aria-label": "Previous path",
                                    onclick: move |_| { let mut c = canvas.write(); c.path_ix = c.path_ix.saturating_sub(1); },
                                    Ico { icon: Icon::ChevronLeft, size: 14 }
                                }
                                span { "{path_ix + 1} of {paths_count}" }
                                button {
                                    class: "icon-btn sm",
                                    disabled: path_ix + 1 >= paths_count,
                                    "aria-label": "Next path",
                                    onclick: move |_| { canvas.write().path_ix += 1; },
                                    Ico { icon: Icon::ChevronRight, size: 14 }
                                }
                            }
                        }
                    }
                    PathSteps { graph: visible.clone(), path: p }
                }
            }
            if flavor == Flavor::Details && !relations.is_empty() {
                div { class: "inspector-section", "Relations" }
                div { class: "relation-list",
                    for (e, other, meta, when) in relations {
                        button {
                            key: "{e.id}",
                            class: "relation-row",
                            title: "Select {other}",
                            onclick: move |_| canvas.write().select(Pick::Node(other.clone())),
                            span { class: "relation-kind", "{e.relation.label()}" }
                            span { class: "relation-other mono", "{step_other(&e, &node.path)}" }
                            span { class: "relation-meta", "{meta}" }
                            span { class: "relation-date", "{when}" }
                        }
                    }
                }
                if let Some(c) = graph_collection {
                    button {
                        class: "link-btn",
                        title: "Open the {c} edges in the Explorer",
                        onclick: open_edges,
                        "View all {total_edges} edges"
                        Ico { icon: Icon::ChevronRight, size: 14 }
                    }
                }
            }
        }
    }
}

/// The id at the far end of `e` from `path`.
fn step_other(e: &GraphEdge, path: &str) -> String {
    let other = if e.from == path { &e.to } else { &e.from };
    other.rsplit('/').next().unwrap_or(other).to_string()
}
