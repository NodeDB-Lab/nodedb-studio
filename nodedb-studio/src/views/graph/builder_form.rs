//! The Builder's form: start node, relationship, end node, options, and the
//! read-only Cypher those controls write.

use dioxus::prelude::*;

use crate::components::clipboard::use_copier;
use crate::components::icon::{Ico, Icon};
use crate::models::console::Protocol;
use crate::models::graph::{NodeKind, Relation};
use crate::models::graph_builder::{Direction, Field, MAX_DEPTH, MAX_LIMIT};
use crate::models::sql_text::tokenize;
use crate::state::graph::{BuilderUi, CanvasState};
use crate::views::graph::toolbar::bindings_resource;

/// Cypher text with its keywords colored, read-only.
#[component]
fn CypherBlock(text: String) -> Element {
    rsx! {
        pre { class: "code-block graph-preview", "aria-label": "Preview Cypher",
            for (i, t) in tokenize(&text, Protocol::Cypher).iter().enumerate() {
                span { key: "{i}", class: "{t.kind.class()}", "{t.text}" }
            }
        }
    }
}

#[component]
fn NodeFields(
    title: &'static str,
    id: &'static str,
    kind: NodeKind,
    kinds: Vec<NodeKind>,
    filter: String,
    error: Option<String>,
    on_kind: EventHandler<NodeKind>,
    on_filter: EventHandler<String>,
) -> Element {
    rsx! {
        div { class: "form-group",
            h3 { class: "form-heading", "{title}" }
            div { class: "field",
                select {
                    id: "{id}-kind",
                    class: "select native",
                    "aria-label": "{title} collection",
                    onchange: move |e| {
                        if let Some(k) = NodeKind::ALL.into_iter().find(|k| k.query_label() == e.value()) {
                            on_kind.call(k);
                        }
                    },
                    for k in kinds {
                        option { key: "{k.label()}", value: "{k.query_label()}", selected: k == kind, "{k.query_label()}" }
                    }
                }
            }
            div { class: "field inline",
                label { r#for: "{id}-filter", "Filter" }
                input {
                    id: "{id}-filter",
                    class: if error.is_some() { "input mono invalid" } else { "input mono" },
                    placeholder: "field = 'value'",
                    value: "{filter}",
                    "aria-invalid": "{error.is_some()}",
                    oninput: move |e| on_filter.call(e.value()),
                }
            }
            if let Some(e) = error.as_ref() {
                div { class: "error", role: "alert", "{e}" }
            }
        }
    }
}

#[component]
pub fn BuilderForm(ui: Signal<BuilderUi>, canvas: Signal<CanvasState>) -> Element {
    let copier = use_copier();
    let u = ui.read().clone();
    let binding = bindings_resource()
        .into_iter()
        .find(|b| b.name == u.binding);
    let kinds = binding
        .as_ref()
        .map(|b| b.kinds.clone())
        .unwrap_or_else(|| NodeKind::ALL.to_vec());
    let relations = binding
        .as_ref()
        .map(|b| b.relations.clone())
        .unwrap_or_else(|| Relation::ALL.to_vec());
    let preview = u.form.cypher();
    let issues = u.form.issues();

    rsx! {
        div { class: "builder-form",
            NodeFields {
                title: "Start node",
                id: "start",
                kind: u.form.start,
                kinds: kinds.clone(),
                filter: u.form.start_filter.clone(),
                error: u.form.issue(Field::StartFilter),
                on_kind: move |k| ui.write().form.start = k,
                on_filter: move |t| ui.write().form.start_filter = t,
            }
            div { class: "form-group",
                h3 { class: "form-heading", "Relationship" }
                div { class: "field",
                    select {
                        class: "select native",
                        "aria-label": "Relationship",
                        onchange: move |e| {
                            if let Some(r) = Relation::from_label(&e.value()) {
                                ui.write().form.relation = r;
                            }
                        },
                        for r in relations {
                            option { key: "{r.label()}", value: "{r.label()}", selected: r == u.form.relation, "{r.label()}" }
                        }
                    }
                }
                div { class: "field inline",
                    label { r#for: "direction", "Direction" }
                    select {
                        id: "direction",
                        class: "select native",
                        onchange: move |e| {
                            if let Some(d) = Direction::ALL.into_iter().find(|d| d.label() == e.value()) {
                                ui.write().form.direction = d;
                            }
                        },
                        for d in Direction::ALL {
                            option { key: "{d.label()}", value: "{d.label()}", selected: d == u.form.direction, "{d.label()}" }
                        }
                    }
                }
                if let Some(e) = u.form.issue(Field::Relation) {
                    div { class: "error", role: "alert", "{e}" }
                }
            }
            NodeFields {
                title: "End node",
                id: "end",
                kind: u.form.end,
                kinds,
                filter: u.form.end_filter.clone(),
                error: u.form.issue(Field::EndFilter),
                on_kind: move |k| ui.write().form.end = k,
                on_filter: move |t| ui.write().form.end_filter = t,
            }
            div { class: "form-group",
                h3 { class: "form-heading", "Options" }
                div { class: "field-pair",
                    div { class: "field inline",
                        label { r#for: "depth", "Max depth" }
                        input {
                            id: "depth",
                            class: if u.form.issue(Field::MaxDepth).is_some() { "input invalid" } else { "input" },
                            r#type: "number",
                            min: "1",
                            max: "{MAX_DEPTH}",
                            value: "{u.form.max_depth}",
                            onchange: move |e| ui.write().form.max_depth = e.value().trim().parse().unwrap_or(0),
                        }
                    }
                    div { class: "field inline",
                        label { r#for: "limit", "Limit" }
                        input {
                            id: "limit",
                            class: if u.form.issue(Field::Limit).is_some() { "input invalid" } else { "input" },
                            r#type: "number",
                            min: "1",
                            max: "{MAX_LIMIT}",
                            value: "{u.form.limit}",
                            onchange: move |e| ui.write().form.limit = e.value().trim().parse().unwrap_or(0),
                        }
                    }
                }
                for f in [Field::MaxDepth, Field::Limit] {
                    if let Some(e) = u.form.issue(f) {
                        div { key: "{e}", class: "error", role: "alert", "{e}" }
                    }
                }
            }
            div { class: "form-group",
                div { class: "form-heading-row",
                    h3 { class: "form-heading", "Preview Cypher" }
                    if let Ok(text) = preview.as_ref() {
                        {
                            let text = text.clone();
                            rsx! {
                                button {
                                    class: "icon-btn sm",
                                    title: "Copy the query",
                                    "aria-label": "Copy the query",
                                    onclick: move |_| copier.copy(text.clone()),
                                    Ico { icon: Icon::Copy, size: 16 }
                                }
                            }
                        }
                    }
                }
                match preview {
                    Ok(text) => rsx! { CypherBlock { text } },
                    Err(_) => rsx! {
                        div { class: "callout warn",
                            Ico { icon: Icon::TriangleAlert, size: 16 }
                            span { "Fix the {issues.len()} marked fields to see the query." }
                        }
                    },
                }
            }
            div { class: "form-actions",
                button {
                    class: "btn primary",
                    disabled: !issues.is_empty(),
                    title: if issues.is_empty() { "Run the previewed query" } else { "Fix the marked fields first" },
                    onclick: move |_| {
                        let started = ui.write().run();
                        if started {
                            canvas.write().reset();
                        }
                    },
                    Ico { icon: Icon::Play, size: 16 }
                    "Run"
                }
            }
        }
    }
}
