//! The record details panel: one inspector per storage profile.
//!
//! Each profile shows the record the way its data reads: JSON for a
//! document, typed fields for a strict row, a value for a key, an
//! embedding's metadata for a vector attribute.

use dioxus::prelude::*;
use nodedb_types::Value;

use crate::components::clipboard::use_copier;
use crate::components::icon::{Ico, Icon};
use crate::components::json_view::JsonView;
use crate::models::collection::{Attached, Collection, Profile};
use crate::models::record_json::{compact, pretty};
use crate::models::records::{CollectionAccess, Record};
use crate::models::schema::CollectionSchema;
use crate::models::search::{SearchMode, VectorInput};
use crate::routes::Route;
use crate::routes::tabs::SearchTab;
use crate::state::explorer::{DetailsTab, RecordView};
use crate::state::search::SearchUi;
use crate::views::explorer::actions::use_launcher;
use crate::views::explorer::cells::{bytes_label, ttl_label};
use crate::views::explorer::details_geo::SpatialDetails;
use crate::views::explorer::details_rows::{ColumnarDetails, SeriesDetails, StrictDetails};
use crate::views::explorer::details_vector::{IndexList, IndexedFields, VectorTabBody};

/// The panel title for a collection.
pub fn details_title(c: &Collection) -> &'static str {
    match c.profile {
        Profile::Document if c.has(Attached::FullText) || c.has(Attached::Vector) => {
            "Document details"
        }
        Profile::Document => "Document",
        Profile::DocumentStrict => "Document details",
        Profile::KeyValue => "Value",
        Profile::Columnar => "Row",
        Profile::Timeseries => "Row inspector",
        Profile::Spatial => "Feature details",
        Profile::Array => "Dimensions & details",
    }
}

/// The tab to show: the chosen one while the profile offers it, else the first.
pub fn effective_tab(chosen: Option<DetailsTab>, tabs: &[DetailsTab]) -> Option<DetailsTab> {
    chosen
        .filter(|t| tabs.contains(t))
        .or_else(|| tabs.first().copied())
}

#[component]
pub fn DetailTabs(tabs: Vec<DetailsTab>, active: DetailsTab) -> Element {
    let mut view = use_context::<Signal<RecordView>>();
    rsx! {
        div { class: "tab-bar sub", role: "tablist",
            for tab in tabs {
                {
                    // Revision history has no design yet.
                    let unavailable = tab == DetailsTab::History;
                    rsx! {
                        button {
                            key: "{tab.label()}",
                            class: if tab == active { "tab active" } else { "tab" },
                            role: "tab",
                            "aria-selected": "{tab == active}",
                            disabled: unavailable,
                            title: if unavailable { "Revision history is not designed yet" } else { "" },
                            onclick: move |_| view.write().details_tab = Some(tab),
                            "{tab.label()}"
                        }
                    }
                }
            }
        }
    }
}

/// Previous and next record of the page.
#[component]
pub fn Stepper(prev: Option<String>, next: Option<String>) -> Element {
    let mut view = use_context::<Signal<RecordView>>();
    let (p, n) = (prev.clone(), next.clone());
    rsx! {
        div { class: "stepper",
            button {
                class: "icon-btn bordered sm",
                disabled: prev.is_none(),
                "aria-label": "Previous record",
                onclick: move |_| view.write().record = p.clone(),
                Ico { icon: Icon::ChevronLeft, size: 16 }
            }
            button {
                class: "icon-btn bordered sm",
                disabled: next.is_none(),
                "aria-label": "Next record",
                onclick: move |_| view.write().record = n.clone(),
                Ico { icon: Icon::ChevronRight, size: 16 }
            }
        }
    }
}

#[component]
pub fn RecordDetails(
    collection: Collection,
    schema: CollectionSchema,
    record: Record,
    access: Option<CollectionAccess>,
    prev: Option<String>,
    next: Option<String>,
    catalog: Vec<String>,
) -> Element {
    match collection.profile {
        Profile::Document
            if collection.has(Attached::FullText) || collection.has(Attached::Vector) =>
        {
            rsx! { RichDocument { collection, schema, record, access, prev, next } }
        }
        Profile::Document => rsx! { PlainDocument { collection, schema, record, access } },
        Profile::DocumentStrict => {
            rsx! { StrictDetails { collection, schema, record, access, catalog } }
        }
        Profile::KeyValue => rsx! { KvDetails { collection, schema, record, access } },
        Profile::Columnar => rsx! { ColumnarDetails { collection, schema, record, access } },
        Profile::Timeseries => {
            rsx! { SeriesDetails { collection, schema, record, access, prev, next } }
        }
        Profile::Spatial => rsx! { SpatialDetails { collection, schema, record, access } },
        Profile::Array => rsx! {},
    }
}

/// Edit, Copy, Delete. A write the session may not make is disabled with
/// the reason.
#[component]
pub fn RecordActions(
    collection: Collection,
    schema: CollectionSchema,
    record: Record,
    access: Option<CollectionAccess>,
    #[props(default = "Edit".to_string())] edit_label: String,
    #[props(default = true)] with_copy: bool,
) -> Element {
    let launcher = use_launcher();
    let copier = use_copier();
    let can = |pick: fn(&CollectionAccess) -> bool| access.as_ref().is_some_and(pick);
    let can_update = can(|a| a.update.allowed());
    let can_delete = can(|a| a.delete.allowed());
    let why_update = access
        .as_ref()
        .and_then(|a| a.update.headline("UPDATE"))
        .unwrap_or_default();
    let why_delete = access
        .as_ref()
        .and_then(|a| a.delete.headline("DELETE"))
        .unwrap_or_default();
    let text = pretty(&record.value, &schema.fields);
    rsx! {
        div { class: "inspector-actions top",
            button {
                class: "btn primary",
                disabled: !can_update,
                title: "{why_update}",
                onclick: {
                    let (c, s, r) = (collection.clone(), schema.clone(), record.clone());
                    move |_| launcher.edit(&c, &s, &r)
                },
                Ico { icon: Icon::Pencil, size: 16 }
                "{edit_label}"
            }
            if with_copy {
                button {
                    class: "btn",
                    onclick: move |_| copier.copy(text.clone()),
                    Ico { icon: Icon::Copy, size: 16 }
                    "Copy"
                }
            }
            button {
                class: "btn danger",
                disabled: !can_delete,
                title: "{why_delete}",
                onclick: {
                    let (c, r) = (collection.name.clone(), record.clone());
                    move |_| launcher.delete_record(&c, &r)
                },
                Ico { icon: Icon::Trash, size: 16 }
                "Delete"
            }
        }
    }
}

/// `id: u44182` for a record whose id lives in a field, else the id alone.
fn heading(schema: &CollectionSchema, record: &Record) -> String {
    match &schema.id_field {
        Some(f) => format!("{f}: {}", record.id),
        None => record.id.clone(),
    }
}

#[component]
fn PlainDocument(
    collection: Collection,
    schema: CollectionSchema,
    record: Record,
    access: Option<CollectionAccess>,
) -> Element {
    let view = use_context::<Signal<RecordView>>();
    let tabs = DetailsTab::for_collection(&collection);
    let active = effective_tab(view.read().details_tab, &tabs).unwrap_or(DetailsTab::Json);
    let head = heading(&schema, &record);
    let text = pretty(&record.value, &schema.fields);
    rsx! {
        div { class: "inspector",
            div { class: "inspector-head",
                h3 { class: "inspector-title", "{head}" }
                span { class: "badge blue", "Document" }
            }
            RecordActions { collection: collection.clone(), schema: schema.clone(), record: record.clone(), access }
            DetailTabs { tabs, active }
            match active {
                DetailsTab::Raw => rsx! {
                    pre { class: "code-block raw-json", "{compact(&record.value, &schema.fields)}" }
                },
                DetailsTab::Json
                | DetailsTab::Document
                | DetailsTab::FullText
                | DetailsTab::Vector
                | DetailsTab::Indexes
                | DetailsTab::Fields
                | DetailsTab::History
                | DetailsTab::Properties
                | DetailsTab::Geometry => rsx! { JsonView { text } },
            }
        }
    }
}

/// A document with Full-text or Vector attached: the source record plus
/// what each capability knows about it.
#[component]
fn RichDocument(
    collection: Collection,
    schema: CollectionSchema,
    record: Record,
    access: Option<CollectionAccess>,
    prev: Option<String>,
    next: Option<String>,
) -> Element {
    let nav = use_navigator();
    let launcher = use_launcher();
    let view = use_context::<Signal<RecordView>>();
    let search = try_use_context::<SearchUi>();
    let tabs = DetailsTab::for_collection(&collection);
    let active = effective_tab(view.read().details_tab, &tabs).unwrap_or(DetailsTab::Document);
    let can_update = access.as_ref().is_some_and(|a| a.update.allowed());
    let can_delete = access.as_ref().is_some_and(|a| a.delete.allowed());
    let text = pretty(&record.value, &schema.fields);
    rsx! {
        div { class: "inspector",
            div { class: "inspector-head split",
                h3 { class: "inspector-title", "{record.id}" }
                if collection.has(Attached::FullText) {
                    button {
                        class: "link-btn",
                        title: "Open Search on this collection, with the Explorer's text filter or this record's title as the query",
                        onclick: {
                            let name = collection.name.clone();
                            let title = match record.get("title") {
                                Some(Value::String(s)) => s.clone(),
                                _ => String::new(),
                            };
                            move |_| {
                                let typed = view.peek().text.trim().to_string();
                                let query = if typed.is_empty() { title.clone() } else { typed };
                                if let Some(search) = search {
                                    search.open_query(&name, SearchMode::Keyword, &query, VectorInput::None);
                                }
                                nav.push(Route::Search { tab: SearchTab::Results });
                            }
                        },
                        Ico { icon: Icon::Search, size: 16 }
                        "Open in Search"
                    }
                }
                Stepper { prev, next }
            }
            div { class: "icon-actions",
                button {
                    class: "btn sm",
                    disabled: !can_update,
                    title: if can_update { "Edit this document" } else { "UPDATE permission required" },
                    onclick: {
                        let (c, s, r) = (collection.clone(), schema.clone(), record.clone());
                        move |_| launcher.edit(&c, &s, &r)
                    },
                    Ico { icon: Icon::Pencil, size: 14 }
                    "Edit"
                }
                button {
                    class: "btn sm danger",
                    disabled: !can_delete,
                    title: if can_delete { "Delete this document" } else { "DELETE permission required" },
                    onclick: {
                        let (c, r) = (collection.name.clone(), record.clone());
                        move |_| launcher.delete_record(&c, &r)
                    },
                    Ico { icon: Icon::Trash, size: 14 }
                    "Delete"
                }
            }
            DetailTabs { tabs, active }
            match active {
                DetailsTab::Document => rsx! {
                    JsonView { text }
                    if let Some(index) = schema.text_index.clone() {
                        IndexedFields { index }
                    }
                },
                DetailsTab::FullText => rsx! {
                    if let Some(index) = schema.text_index.clone() {
                        IndexedFields { index }
                    }
                },
                DetailsTab::Vector => rsx! {
                    VectorTabBody { schema: schema.clone(), record: record.clone() }
                },
                DetailsTab::Indexes => rsx! { IndexList { indexes: schema.indexes.clone() } },
                DetailsTab::Json | DetailsTab::Raw | DetailsTab::Fields | DetailsTab::History
                | DetailsTab::Properties | DetailsTab::Geometry => rsx! { JsonView { text } },
            }
        }
    }
}

#[component]
fn KvDetails(
    collection: Collection,
    schema: CollectionSchema,
    record: Record,
    access: Option<CollectionAccess>,
) -> Element {
    let launcher = use_launcher();
    let copier = use_copier();
    let can_update = access.as_ref().is_some_and(|a| a.update.allowed());
    let can_delete = access.as_ref().is_some_and(|a| a.delete.allowed());
    let value = record.get("value").cloned().unwrap_or_default();
    let kind = record
        .get("type")
        .map(crate::views::explorer::cells::text_label)
        .unwrap_or_default();
    let text = match &value {
        nodedb_types::Value::Bytes(b) => format!("<{} bytes>", b.len()),
        other => pretty(other, &[]),
    };
    let size = record
        .get("size")
        .and_then(|v| v.as_i64())
        .map(bytes_label)
        .unwrap_or_default();
    let ttl = ttl_label(record.get("ttl").and_then(|v| v.as_i64()));
    let badge = if kind == "json" {
        "JSON".to_string()
    } else {
        kind.clone()
    };
    rsx! {
        div { class: "inspector",
            div { class: "inspector-head",
                h3 { class: "inspector-title mono", "{record.id}" }
                span { class: "badge blue", "{badge}" }
            }
            JsonView { text: text.clone() }
            div { class: "kv kv-compact",
                div { class: "kv-row", span { "Size" } span { "{size}" } }
                div { class: "kv-row", span { "TTL" } span { "{ttl}" } }
            }
            div { class: "stack-actions",
                button {
                    class: "btn primary block",
                    disabled: !can_update,
                    title: if can_update { "" } else { "UPDATE permission required" },
                    onclick: {
                        let (c, s, r) = (collection.clone(), schema.clone(), record.clone());
                        move |_| launcher.edit(&c, &s, &r)
                    },
                    Ico { icon: Icon::Pencil, size: 16 }
                    "Edit value"
                }
                button {
                    class: "btn block",
                    disabled: true,
                    title: "Setting an expiry on its own is not designed yet: edit the entry to change `ttl`",
                    Ico { icon: Icon::Clock, size: 16 }
                    "Set TTL"
                }
                button {
                    class: "btn block danger",
                    disabled: !can_delete,
                    title: if can_delete { "" } else { "DELETE permission required" },
                    onclick: {
                        let (c, r) = (collection.name.clone(), record.clone());
                        move |_| launcher.delete_record(&c, &r)
                    },
                    Ico { icon: Icon::Trash, size: 16 }
                    "Delete key"
                }
                button {
                    class: "btn block",
                    onclick: move |_| copier.copy(text.clone()),
                    Ico { icon: Icon::Copy, size: 16 }
                    "Copy"
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tab_the_profile_lacks_falls_back_to_the_first() {
        let tabs = [DetailsTab::Json, DetailsTab::Raw];
        assert_eq!(
            effective_tab(Some(DetailsTab::Raw), &tabs),
            Some(DetailsTab::Raw)
        );
        assert_eq!(
            effective_tab(Some(DetailsTab::Vector), &tabs),
            Some(DetailsTab::Json)
        );
        assert_eq!(effective_tab(None, &tabs), Some(DetailsTab::Json));
        assert_eq!(effective_tab(None, &[]), None);
    }
}
