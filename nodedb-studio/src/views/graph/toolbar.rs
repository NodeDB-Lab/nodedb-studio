//! Toolbar pieces the Cypher and Builder tabs share: the graph binding, its
//! whole-graph totals, and the layout that arranges graph nodes.

use std::rc::Rc;
use std::str::FromStr;

use dioxus::prelude::*;

use crate::models::collection::Collection;
use crate::models::format::compact;
use crate::models::graph::GraphBinding;
use crate::models::graph_layout::LayoutKind;
use crate::services::backend::Backend;

/// A number out of a `<select>` value.
pub fn parse_choice<T: FromStr>(text: &str) -> Option<T> {
    text.parse().ok()
}

/// The graph bindings, re-read when the connection changes.
pub fn bindings_resource() -> Vec<GraphBinding> {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let list = use_resource(move || {
        let backend = backend.clone();
        let _conn = conn();
        async move { backend.graph_bindings().await }
    });
    let read = list.read().clone();
    read.and_then(Result::ok).unwrap_or_default()
}

/// The Explorer's catalog: which records open there.
pub fn catalog_resource() -> Vec<Collection> {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let list = use_resource(move || {
        let backend = backend.clone();
        let _conn = conn();
        async move { backend.catalog().await }
    });
    let read = list.read().clone();
    read.and_then(Result::ok).unwrap_or_default()
}

/// The graph a query runs on. Every graph read names it.
#[component]
pub fn BindingSelect(
    value: String,
    on_change: EventHandler<String>,
    #[props(default = "Graph binding".to_string())] label: String,
) -> Element {
    let mut names: Vec<String> = bindings_resource().into_iter().map(|b| b.name).collect();
    if !names.contains(&value) {
        names.push(value.clone());
    }
    rsx! {
        select {
            class: "select native compact graph-source",
            "aria-label": "{label}",
            title: "{label}",
            value: "{value}",
            onchange: move |e| on_change.call(e.value()),
            for n in names {
                option { key: "{n}", value: "{n}", selected: n == value, "{n}" }
            }
        }
    }
}

/// The whole graph's size, never the visible result's.
#[component]
pub fn Totals(binding: String) -> Element {
    let found = bindings_resource().into_iter().find(|b| b.name == binding);
    rsx! {
        if let Some(b) = found {
            span { class: "graph-totals", title: "Whole {b.name}, not the visible result",
                "{compact(b.nodes)} nodes · {compact(b.edges)} edges"
            }
        }
    }
}

/// How graph nodes are arranged. It never moves a panel.
#[component]
pub fn LayoutSelect(value: LayoutKind, on_change: EventHandler<LayoutKind>) -> Element {
    rsx! {
        select {
            class: "select native compact",
            "aria-label": "Layout",
            title: "Arranges graph nodes. Panels dock through the View menu.",
            value: "{value.label()}",
            onchange: move |e| {
                if let Some(k) = LayoutKind::ALL.into_iter().find(|k| k.label() == e.value()) {
                    on_change.call(k);
                }
            },
            for k in LayoutKind::ALL {
                option { key: "{k.label()}", value: "{k.label()}", selected: k == value, "Layout: {k.label()}" }
            }
        }
    }
}
