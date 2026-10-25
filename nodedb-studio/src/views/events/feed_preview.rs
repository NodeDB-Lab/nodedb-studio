//! The feed drawer's preview column and its review step.

use dioxus::prelude::*;

use crate::components::async_view::AsyncView;
use crate::components::icon::{Ico, Icon};
use crate::components::json_view::JsonView;
use crate::models::feed_draft::{CheckStatus, FeedPreview, SchemaNode};
use crate::models::feeds::ChangeFeed;
use crate::services::async_state::AsyncState;
use crate::state::events::EventsUi;
use crate::state::events::feeds::{EditorMode, FeedEditor};

fn toggle(mut ui: Signal<crate::state::events::feeds::FeedsUi>, path: String) {
    if let Some(e) = ui.write().editor.as_mut() {
        if let Some(i) = e.open_nodes.iter().position(|p| *p == path) {
            e.open_nodes.remove(i);
        } else {
            e.open_nodes.push(path);
        }
    }
}

#[component]
fn SchemaTree(nodes: Vec<SchemaNode>, open: Vec<String>, parent: String, depth: usize) -> Element {
    let ui = use_context::<EventsUi>().feeds;
    rsx! {
        for n in nodes {
            {
                let path = if parent.is_empty() { n.name.clone() } else { format!("{parent}.{}", n.name) };
                let expandable = !n.children.is_empty();
                let is_open = open.contains(&path);
                let pad = format!("padding-left: {}px", depth * 18);
                rsx! {
                    div { key: "{path}", class: "schema-node",
                        div { class: "schema-row", style: "{pad}",
                            if expandable {
                                button {
                                    class: "icon-btn sm schema-toggle",
                                    "aria-expanded": "{is_open}",
                                    "aria-label": if is_open { "Collapse {n.name}" } else { "Expand {n.name}" },
                                    onclick: {
                                        let path = path.clone();
                                        move |_| toggle(ui, path.clone())
                                    },
                                    Ico { icon: if is_open { Icon::ChevronDown } else { Icon::ChevronRight }, size: 16 }
                                }
                            } else {
                                span { class: "schema-toggle spacer" }
                            }
                            span { class: "schema-name", "{n.name}" }
                            span { class: "schema-type", "{n.ty}" }
                            if let Some(v) = n.value.clone() {
                                span { class: "schema-value", "\"{v}\"" }
                            }
                        }
                        if expandable && is_open {
                            SchemaTree {
                                nodes: n.children.clone(),
                                open: open.clone(),
                                parent: path.clone(),
                                depth: depth + 1,
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Preview schema and Sample event, read from the server for the draft.
#[component]
pub fn PreviewColumn(state: AsyncState<FeedPreview>, open: Vec<String>) -> Element {
    rsx! {
        div { class: "sheet-col preview-col",
            h3 { class: "section-title", "Preview schema" }
            AsyncView {
                loading: state.is_loading(),
                empty: false,
                error: state.error_message(),
                retriable: false,
            }
            if let Some(p) = state.loaded() {
                div { class: "schema-tree", role: "tree", "aria-label": "Event schema",
                    SchemaTree { nodes: p.schema.clone(), open: open.clone(), parent: String::new(), depth: 0 }
                }
                h3 { class: "section-title", "Sample event" }
                JsonView { text: p.sample.clone(), class: "sample-json" }
            }
        }
    }
}

/// The review: what will be created or changed, what the server checked, and
/// what at-least-once delivery means for the consumer.
#[component]
pub fn ReviewStep(
    editor: FeedEditor,
    original: Option<ChangeFeed>,
    preview: AsyncState<FeedPreview>,
) -> Element {
    let d = editor.draft.clone();
    let changes = original.as_ref().map(|f| d.changes_from(f));
    let editing = matches!(editor.mode, EditorMode::Edit { .. });
    rsx! {
        div { class: "review-feed",
            if editing {
                div { class: "review-block",
                    h3 { class: "section-title", "Changes to {d.name}" }
                    if let Some(rows) = changes.clone() {
                        if rows.is_empty() {
                            div { class: "insp-sub", "No field differs from the saved feed." }
                        }
                        div { class: "kv",
                            for (label, was, now) in rows {
                                div { key: "{label}", class: "kv-row",
                                    span { "{label}" }
                                    span { class: "change-values",
                                        span { class: "was", "{was}" }
                                        Ico { icon: Icon::ChevronRight, size: 16 }
                                        span { class: "now", "{now}" }
                                    }
                                }
                            }
                        }
                    }
                    if let EditorMode::Edit { revision, .. } = &editor.mode {
                        div { class: "insp-sub",
                            "Saved against revision {revision}. If someone else changed {d.name} since, the save is refused and nothing is overwritten."
                        }
                    }
                }
            } else {
                div { class: "review-block",
                    h3 { class: "section-title", "{d.name} will capture" }
                    div { class: "kv",
                        div { class: "kv-row", span { "Source" } span { class: "mono", "{d.source}" } }
                        div { class: "kv-row",
                            span { "Operations" }
                            span { {d.operations.iter().map(|o| o.label()).collect::<Vec<_>>().join(" · ")} }
                        }
                        div { class: "kv-row", span { "Filter" } span { class: "mono", if d.filter.trim().is_empty() { "None" } else { "{d.filter.trim()}" } } }
                        div { class: "kv-row", span { "Projection" } span { class: "mono", if d.projection.trim().is_empty() { "Whole document" } else { "{d.projection.trim()}" } } }
                        div { class: "kv-row", span { "Before image" } span { if d.before_image { "Included" } else { "Not included" } } }
                        div { class: "kv-row", span { "Start position" } span { "{d.start.label()}" } }
                        div { class: "kv-row", span { "Consumer" } span { "{d.alias} · {d.delivery.consumer_label()}" } }
                        if d.delivery == crate::models::feeds::DeliveryKind::Webhook {
                            div { class: "kv-row", span { "Endpoint" } span { class: "mono", "{d.endpoint}" } }
                        }
                        div { class: "kv-row", span { "Retries" } span { "{d.backoff.label()}, up to {d.max_retries}" } }
                        div { class: "kv-row", span { "Retention" } span { "{d.retention} {d.retention_unit.label().to_lowercase()}" } }
                    }
                }
            }
            div { class: "review-block",
                h3 { class: "section-title", "Server checks" }
                AsyncView {
                    loading: preview.is_loading(),
                    empty: false,
                    error: preview.error_message(),
                    retriable: false,
                }
                if let Some(p) = preview.loaded() {
                    div { class: "check-rows",
                        for c in p.checks.iter() {
                            {
                                let (class, icon) = match c.status {
                                    CheckStatus::Pass => ("review-check ok", Icon::CircleCheck),
                                    CheckStatus::Warn => ("review-check warn", Icon::TriangleAlert),
                                    CheckStatus::Fail => ("review-check err", Icon::CircleAlert),
                                };
                                rsx! {
                                    div { key: "{c.label}", class: "{class}",
                                        Ico { icon, size: 16 }
                                        div { class: "check-text",
                                            strong { "{c.label}" }
                                            span { class: "sr-only", " {c.status.label()}." }
                                            span { class: "insp-sub", "{c.detail}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "callout info",
                Ico { icon: Icon::Info, size: 16 }
                span {
                    strong { "At-least-once delivery. " }
                    "{d.alias} can receive an event more than once, including after a retry or a replay. Make it duplicate-aware. A delivery acknowledgment is not a Lite sync acknowledgment."
                }
            }
        }
    }
}
