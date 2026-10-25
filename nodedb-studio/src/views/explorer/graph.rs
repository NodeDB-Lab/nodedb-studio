//! The Graph tab of a collection with a graph attached: its edges, and the
//! details of the selected edge.
//!
//! A graph is an overlay on records. The Nodes view is the collection's own
//! records, so it shows the Data body. Traversal and queries stay in Graph
//! Explorer.

use dioxus::prelude::*;

use crate::components::clipboard::use_copier;
use crate::components::icon::{Ico, Icon};
use crate::components::json_view::JsonView;
use crate::components::pager::Pager;
use crate::models::collection::Collection;
use crate::models::format::{date_second, thousands};
use crate::models::graph_records::{Edge, EdgePage, NodeRef};
use crate::models::record_json::pretty;
use crate::models::records::CollectionAccess;
use crate::models::schema::CollectionSchema;
use crate::routes::Route;
use crate::routes::tabs::GraphTab;
use crate::services::async_state::AsyncState;
use crate::state::explorer::{GraphSub, RecordView, Selected, select_record};
use crate::state::preferences::Preferences;
use crate::views::explorer::actions::use_launcher;
use crate::views::explorer::links::OpenLink;
use nodedb_types::Value;

/// Edges per page.
pub const EDGE_PAGE: usize = 10;

fn created(edge: &Edge, offset: i16) -> String {
    match &edge.properties {
        Value::Object(m) => match m.get("created_at") {
            Some(Value::DateTime(t)) => date_second(t.unix_secs(), offset),
            Some(other) => crate::views::explorer::cells::text_label(other),
            None => String::new(),
        },
        _ => String::new(),
    }
}

fn node_icon(node: &NodeRef) -> Icon {
    if node.kind == "User" {
        Icon::User
    } else {
        Icon::Box
    }
}

/// An endpoint as a link into the Data tab of its collection.
#[component]
fn NodeLink(node: NodeRef) -> Element {
    let mut selected = use_context::<Signal<Option<Selected>>>();
    let mut view = use_context::<Signal<RecordView>>();
    let (c, id) = (node.collection.clone(), node.id.clone());
    rsx! {
        button {
            class: "link-btn plain",
            title: "Open {node.path()}",
            onclick: move |e| {
                e.stop_propagation();
                select_record(&mut selected, &mut view, &c, &id);
            },
            "{node.path()}"
        }
    }
}

#[component]
pub fn GraphBody(
    collection: Collection,
    schema: CollectionSchema,
    binding: String,
    edges: AsyncState<EdgePage>,
    /// The collection's records, which are the graph's nodes.
    nodes: Element,
    selected_edge: Option<String>,
    access: Option<CollectionAccess>,
    on_retry: EventHandler<()>,
) -> Element {
    let mut view = use_context::<Signal<RecordView>>();
    let prefs = use_context::<Signal<Preferences>>();
    let launcher = use_launcher();
    let offset = prefs.read().utc_offset_minutes;
    let sub = view.read().graph_sub;
    let relation = view.read().relation.clone();
    let edge_page = view.read().edge_page;
    let can_create = access.as_ref().is_some_and(|a| a.create.allowed());
    let loaded = edges.loaded().cloned();
    let relations = loaded
        .as_ref()
        .map(|p| p.relations.clone())
        .unwrap_or_default();

    rsx! {
        div { class: "graph-meta",
            div { class: "field",
                span { class: "field-label", "Source collection" }
                span { class: "meta-value", "{collection.name}" }
            }
            div { class: "field",
                span { class: "field-label", "Graph binding" }
                span { class: "meta-value mono", "{binding}" }
            }
        }
        div { class: "tab-bar sub graph-tabs", role: "tablist",
            for s in [GraphSub::Nodes, GraphSub::Edges] {
                button {
                    key: "{s:?}",
                    class: if s == sub { "tab active" } else { "tab" },
                    role: "tab",
                    "aria-selected": "{s == sub}",
                    onclick: move |_| view.write().graph_sub = s,
                    match s { GraphSub::Nodes => "Nodes", GraphSub::Edges => "Edges" }
                }
            }
        }
        match sub {
            GraphSub::Nodes => nodes,
            GraphSub::Edges => rsx! {
                div { class: "toolbar records-toolbar",
                    label { class: "toolbar-select",
                        span { "Relation" }
                        select {
                            class: "select native compact",
                            "aria-label": "Relation",
                            onchange: move |e| {
                                let v = e.value();
                                let mut w = view.write();
                                w.relation = (v != "*").then_some(v);
                                w.edge_page = 0;
                                w.edge = None;
                            },
                            option { value: "*", selected: relation.is_none(), "All relations" }
                            for r in relations {
                                option { key: "{r}", value: "{r}", selected: relation.as_deref() == Some(r.as_str()), "{r}" }
                            }
                        }
                    }
                    div { class: "search-field grow",
                        Ico { icon: Icon::Search, size: 16 }
                        input {
                            class: "input",
                            placeholder: "Filter edges…",
                            "aria-label": "Filter edges",
                            value: "{view.read().edge_text}",
                            oninput: move |e| {
                                let mut w = view.write();
                                w.edge_text = e.value();
                                w.edge_page = 0;
                                w.edge = None;
                            },
                        }
                    }
                    div { class: "toolbar-spacer" }
                    button {
                        class: "btn primary",
                        disabled: !can_create,
                        title: if can_create { "" } else { "INSERT permission required" },
                        onclick: {
                            let (c, s) = (collection.clone(), schema.clone());
                            move |_| launcher.insert(&c, &s)
                        },
                        Ico { icon: Icon::Plus, size: 16 }
                        "Node"
                    }
                    button {
                        class: "btn primary",
                        disabled: true,
                        title: "Creating an edge is not designed yet",
                        Ico { icon: Icon::Plus, size: 16 }
                        "Edge"
                    }
                    OpenLink {
                        route: Route::DataTransfer {},
                        label: "Export".to_string(),
                        icon: Icon::Upload,
                        class: "btn".to_string(),
                    }
                }
                crate::components::async_view::AsyncView {
                    loading: edges.is_loading(),
                    empty: false,
                    error: edges.error_message(),
                    retriable: edges.is_retriable(),
                    on_retry: move |_| on_retry.call(()),
                }
                if let Some(page) = loaded {
                    div { class: "dt-wrap records-wrap",
                        table { class: "dt records-table",
                            thead {
                                tr {
                                    th { class: "check-col" }
                                    th { "Edge id" }
                                    th { "Relation" }
                                    th { "From" }
                                    th { "To" }
                                    th { "Properties" }
                                }
                            }
                            tbody {
                                for e in page.edges.iter() {
                                    {
                                        let id = e.id.clone();
                                        let is_selected = selected_edge.as_deref() == Some(e.id.as_str());
                                        rsx! {
                                            tr {
                                                key: "{e.id}",
                                                class: if is_selected { "clickable selected" } else { "clickable" },
                                                "aria-selected": "{is_selected}",
                                                onclick: move |_| view.write().edge = Some(id.clone()),
                                                td { class: "check-col",
                                                    input { r#type: "checkbox", class: "checkbox", checked: is_selected, "aria-label": "Select {e.id}", onclick: move |e| e.stop_propagation() }
                                                }
                                                td { class: "mono", "{e.id}" }
                                                td { span { class: "badge blue relation", "{e.relation}" } }
                                                td { NodeLink { node: e.from.clone() } }
                                                td { NodeLink { node: e.to.clone() } }
                                                td { "{created(e, offset)}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        if page.edges.is_empty() {
                            div { class: "empty-state records-empty",
                                Ico { icon: Icon::GitFork, size: 32 }
                                h3 { "No edges match" }
                                p { "No edges in {binding} match the relation and text filter." }
                            }
                        }
                    }
                    div { class: "dt-footer records-footer",
                        span { class: "records-count",
                            if page.edges.is_empty() {
                                "No matching edges"
                            } else {
                                "{edge_page * EDGE_PAGE + 1}–{edge_page * EDGE_PAGE + page.edges.len()} of {thousands(page.total)} edges"
                            }
                        }
                        Pager {
                            page: edge_page,
                            pages: usize::try_from(page.total.div_ceil(EDGE_PAGE as u64)).unwrap_or(0),
                            on_page: move |p| {
                                let mut w = view.write();
                                w.edge_page = p;
                                w.edge = None;
                            },
                        }
                    }
                }
            },
        }
    }
}

#[component]
fn Endpoint(label: &'static str, node: NodeRef) -> Element {
    rsx! {
        div { class: "endpoint",
            span { class: "endpoint-label", "{label}" }
            div { class: "endpoint-body",
                div { class: "endpoint-link",
                    Ico { icon: node_icon(&node), size: 18 }
                    NodeLink { node: node.clone() }
                }
                div { class: "kv kv-compact",
                    div { class: "kv-row", span { "Collection" } span { "{node.collection}" } }
                    div { class: "kv-row", span { "Type" } span { "{node.kind}" } }
                }
            }
        }
    }
}

#[component]
pub fn EdgeDetails(edge: Edge, binding: String, access: Option<CollectionAccess>) -> Element {
    let launcher = use_launcher();
    let copier = use_copier();
    let can_delete = access.as_ref().is_some_and(|a| a.update.allowed());
    let props = pretty(&edge.properties, &[]);
    rsx! {
        div { class: "inspector",
            div { class: "kv kv-compact",
                div { class: "kv-row",
                    span { "Edge id" }
                    span { class: "with-icon mono", "{edge.id}"
                        button {
                            class: "icon-btn sm",
                            title: "Copy edge id",
                            "aria-label": "Copy edge id",
                            onclick: {
                                let id = edge.id.clone();
                                move |_| copier.copy(id.clone())
                            },
                            Ico { icon: Icon::Copy, size: 14 }
                        }
                    }
                }
                div { class: "kv-row", span { "Relation" } span { class: "badge blue relation", "{edge.relation}" } }
            }
            h4 { class: "inspector-section", "Endpoints" }
            Endpoint { label: "From", node: edge.from.clone() }
            Endpoint { label: "To", node: edge.to.clone() }
            div { class: "card-head",
                h4 { class: "inspector-section", "Properties" }
                button {
                    class: "icon-btn sm",
                    title: "Copy properties",
                    "aria-label": "Copy properties",
                    onclick: {
                        let t = props.clone();
                        move |_| copier.copy(t.clone())
                    },
                    Ico { icon: Icon::Copy, size: 14 }
                }
            }
            JsonView { text: props.clone() }
            div { class: "button-pair",
                button {
                    class: "btn",
                    disabled: true,
                    title: "Editing an edge is not designed yet",
                    Ico { icon: Icon::Pencil, size: 16 }
                    "Edit edge"
                }
                button {
                    class: "btn danger",
                    disabled: !can_delete,
                    title: if can_delete { "" } else { "DELETE permission required" },
                    onclick: {
                        let (g, id) = (binding.clone(), edge.id.clone());
                        move |_| launcher.delete_edge(&g, &id)
                    },
                    Ico { icon: Icon::Trash, size: 16 }
                    "Delete edge"
                }
            }
            OpenLink {
                route: Route::Graph { tab: GraphTab::Cypher },
                label: "Open in Graph Explorer".to_string(),
                icon: Icon::GitFork,
                class: "btn block".to_string(),
            }
        }
    }
}
