//! Details that belong to attached capabilities: the full-text index, the
//! vector attribute, and a collection's index list.

use dioxus::prelude::*;
use nodedb_types::Value;

use crate::components::icon::Icon;
use crate::models::format::offset_label;
use crate::models::records::Record;
use crate::models::schema::{CollectionSchema, IndexDef, TextIndex};
use crate::routes::Route;
use crate::routes::tabs::VectorTab;
use crate::state::explorer::{CollectionTab, RecordView};
use crate::state::preferences::Preferences;
use crate::views::explorer::cells::{text_label, time_label};
use crate::views::explorer::links::OpenLink;

fn status(ready: bool) -> Element {
    let (dot, label) = if ready {
        ("status-dot ok", "Ready")
    } else {
        ("status-dot warn", "Building")
    };
    rsx! { span { class: "status", span { class: "{dot}" } "{label}" } }
}

/// The text fields a full-text index covers, with its status. Coverage the
/// server does not report reads `Not reported`: Ready alone never proves the
/// index holds every record.
#[component]
pub fn IndexedFields(index: TextIndex) -> Element {
    let coverage = index
        .coverage_percent
        .map_or_else(|| "Not reported".to_string(), |p| format!("{p}%"));
    rsx! {
        div { class: "index-block",
            div { class: "index-head",
                h4 { class: "inspector-section", "Indexed fields (Full-text)" }
                OpenLink { route: Route::Designer {}, label: "Edit".to_string(), icon: Icon::Pencil, class: "btn sm".to_string() }
            }
            table { class: "dt dt-compact",
                thead { tr { th { "Field" } th { "Analyzer" } } }
                tbody {
                    for (field, analyzer) in index.fields.iter() {
                        tr { key: "{field}", td { "{field}" } td { "{analyzer}" } }
                    }
                }
            }
            div { class: "kv kv-compact",
                div { class: "kv-row", span { "Index" } span { class: "mono", "{index.name}" } }
                div { class: "kv-row", span { "Index status" } {status(index.ready)} }
                div { class: "kv-row", span { "Coverage" } span { "{coverage}" } }
            }
        }
    }
}

#[component]
pub fn IndexList(indexes: Vec<IndexDef>) -> Element {
    rsx! {
        div { class: "index-block",
            table { class: "dt dt-compact",
                thead { tr { th { "Index" } th { "Kind" } th { "Fields" } th { "State" } } }
                tbody {
                    for ix in indexes.iter() {
                        tr { key: "{ix.name}",
                            td { class: "mono", "{ix.name}" }
                            td { "{ix.kind.label()}" }
                            td { "{ix.fields.join(\", \")}" }
                            td { {status(ix.ready)} }
                        }
                    }
                }
            }
        }
    }
}

/// The Vector tab of a document with a vector attribute.
#[component]
pub fn VectorTabBody(schema: CollectionSchema, record: Record) -> Element {
    let mut view = use_context::<Signal<RecordView>>();
    let prefs = use_context::<Signal<Preferences>>();
    let mut full = use_signal(|| false);
    let Some(vector) = schema.vector.clone() else {
        return rsx! {};
    };
    let zone = offset_label(prefs.read().utc_offset_minutes);
    let embedded = record
        .get("embedded_at")
        .map(|v| format!("{} {zone}", time_label(v, prefs.read().utc_offset_minutes)))
        .unwrap_or_default();
    let model = record
        .get("model")
        .map(text_label)
        .unwrap_or_else(|| vector.model.clone());
    let components = match record.get(&vector.field) {
        Some(Value::Vector(v)) => Some(v.clone()),
        _ => None,
    };
    let dimensions = components
        .as_ref()
        .map_or(vector.dimensions as usize, |v| v.len());
    let all_values = components
        .as_ref()
        .map(|v| {
            v.iter()
                .map(|f| format!("{f:.4}"))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    rsx! {
        div { class: "card",
            h4 { class: "inspector-section", "Embedding metadata" }
            div { class: "kv kv-compact",
                div { class: "kv-row", span { "Dimensions" } span { "{dimensions}" } }
                div { class: "kv-row", span { "Normalized" } span { if vector.normalized { "Yes" } else { "No" } } }
                div { class: "kv-row", span { "Model" } span { "{model}" } }
                div { class: "kv-row", span { "Model revision" } span { "{vector.model_revision}" } }
                div { class: "kv-row", span { "Embedded at" } span { "{embedded}" } }
            }
            div { class: "stack-actions",
                button {
                    class: "btn block",
                    "aria-expanded": "{full()}",
                    disabled: components.is_none(),
                    onclick: move |_| full.set(!full()),
                    crate::components::icon::Ico { icon: Icon::List, size: 16 }
                    if full() { "Hide full vector" } else { "View full vector" }
                }
                if full() {
                    pre { class: "code-block full-vector", "[{all_values}]" }
                }
                div { class: "button-pair",
                    button {
                        class: "btn",
                        disabled: true,
                        title: "Replacing an embedding needs a model and dimension compatibility check, which is not designed yet",
                        crate::components::icon::Ico { icon: Icon::RefreshCw, size: 16 }
                        "Replace value"
                    }
                    OpenLink {
                        route: Route::Search { tab: crate::routes::tabs::SearchTab::Results },
                        label: "Open Semantic Search".to_string(),
                        icon: Icon::Search,
                        class: "btn".to_string(),
                    }
                }
                OpenLink {
                    route: Route::Vector { tab: VectorTab::Projection },
                    label: "Open projection".to_string(),
                    icon: Icon::GitFork,
                    class: "btn block".to_string(),
                }
            }
        }
        div { class: "card",
            h4 { class: "inspector-section", "Configuration" }
            div { class: "kv kv-compact",
                div { class: "kv-row",
                    span { "Source field" }
                    button {
                        class: "link-btn",
                        onclick: move |_| view.write().tab = CollectionTab::Schema,
                        "{vector.field}"
                    }
                }
                div { class: "kv-row",
                    span { "Index" }
                    OpenLink {
                        route: Route::Vector { tab: VectorTab::Index },
                        label: vector.index.clone(),
                        icon: Icon::Grid3x3,
                    }
                }
            }
        }
    }
}
