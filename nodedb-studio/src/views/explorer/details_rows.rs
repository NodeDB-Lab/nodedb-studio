//! Details of typed rows: strict documents, columnar rows, timeseries rows.

use dioxus::prelude::*;
use nodedb_types::Value;

use crate::components::clipboard::use_copier;
use crate::components::icon::{Ico, Icon};
use crate::models::collection::Collection;
use crate::models::format::offset_label;
use crate::models::records::{CollectionAccess, Grant, Record};
use crate::models::schema::{CollectionSchema, FieldType};
use crate::routes::Route;
use crate::routes::tabs::{AdminTab, TimeseriesTab};
use crate::state::explorer::{DetailsTab, RecordView, Selected, select_record};
use crate::state::preferences::Preferences;
use crate::state::timeseries::TimeseriesUi;
use crate::views::explorer::cells::{status_tone, text_label, time_label};
use crate::views::explorer::details::{DetailTabs, RecordActions, Stepper, effective_tab};

/// One field value as a details row shows it.
fn value_text(record: &Record, field: &str, offset: i16) -> String {
    match record.get(field) {
        None | Some(Value::Null) => "–".to_string(),
        Some(v @ Value::DateTime(_)) => time_label(v, offset),
        Some(Value::Array(items)) => format!("[ {} items ]", items.len()),
        Some(Value::Object(map)) => format!("{{ {} fields }}", map.len()),
        Some(v) => text_label(v),
    }
}

/// `customers` → `customer`.
fn singular(name: &str) -> String {
    name.strip_suffix('s').unwrap_or(name).to_string()
}

#[component]
pub fn StrictDetails(
    collection: Collection,
    schema: CollectionSchema,
    record: Record,
    access: Option<CollectionAccess>,
    catalog: Vec<String>,
) -> Element {
    let launcher = crate::views::explorer::actions::use_launcher();
    let mut selected = use_context::<Signal<Option<Selected>>>();
    let mut view = use_context::<Signal<RecordView>>();
    let prefs = use_context::<Signal<Preferences>>();
    let offset = prefs.read().utc_offset_minutes;
    let can_update = access.as_ref().is_some_and(|a| a.update.allowed());
    let can_delete = access.as_ref().is_some_and(|a| a.delete.allowed());
    // The first logical reference names the record the link opens.
    let related = schema
        .fields
        .iter()
        .find_map(|f| f.reference.clone().map(|target| (f.name.clone(), target)));
    let related_id = related
        .as_ref()
        .and_then(|(field, _)| record.get(field).map(text_label));
    let related_known = related
        .as_ref()
        .is_some_and(|(_, target)| catalog.contains(target));
    let (link_label, link_title) = match &related {
        Some((_, target)) if related_known => {
            (format!("View related {}", singular(target)), String::new())
        }
        Some((_, target)) => (
            format!("View related {}", singular(target)),
            format!("{target} is not in this database's catalog"),
        ),
        None => (
            "View related record".to_string(),
            "This collection declares no reference".to_string(),
        ),
    };
    rsx! {
        div { class: "inspector",
            div { class: "inspector-head",
                h3 { class: "inspector-title", "{record.id}" }
                span { class: "badge blue", "Document" }
            }
            div { class: "inspector-actions top",
                button {
                    class: "btn primary",
                    disabled: !can_update,
                    title: if can_update { "" } else { "UPDATE permission required" },
                    onclick: {
                        let (c, s, r) = (collection.clone(), schema.clone(), record.clone());
                        move |_| launcher.edit(&c, &s, &r)
                    },
                    Ico { icon: Icon::Pencil, size: 16 }
                    "Edit record"
                }
                button {
                    class: "btn",
                    disabled: !related_known || related_id.is_none(),
                    title: "{link_title}",
                    onclick: {
                        let target = related.as_ref().map(|(_, t)| t.clone());
                        let id = related_id.clone();
                        move |_| {
                            if let (Some(t), Some(id)) = (&target, &id) {
                                select_record(&mut selected, &mut view, t, id);
                            }
                        }
                    },
                    Ico { icon: Icon::ExternalLink, size: 16 }
                    "{link_label}"
                }
                button {
                    class: "btn danger icon-only",
                    disabled: !can_delete,
                    title: if can_delete { "Delete this record" } else { "DELETE permission required" },
                    "aria-label": "Delete record",
                    onclick: {
                        let (c, r) = (collection.name.clone(), record.clone());
                        move |_| launcher.delete_record(&c, &r)
                    },
                    Ico { icon: Icon::Trash, size: 16 }
                }
            }
            div { class: "fields-head",
                h4 { class: "inspector-section", "Fields" }
                span { class: "fields-revision", "Revision {record.revision}" }
            }
            div { class: "field-list",
                for f in schema.fields.iter() {
                    {
                        let text = value_text(&record, &f.name, offset);
                        let structured = matches!(f.ty, FieldType::Array | FieldType::Object);
                        let is_status = f.name == "status";
                        let tone = status_tone(&text);
                        rsx! {
                            div { key: "{f.name}", class: "field-row",
                                span { class: "field-name",
                                    "{f.name}"
                                    if f.required { span { class: "required", " *" } }
                                }
                                span { class: "field-type", "{f.ty.label()}" }
                                span { class: if structured { "field-value link" } else { "field-value" },
                                    if is_status {
                                        span { class: "badge {tone}", "{text}" }
                                    } else {
                                        "{text}"
                                    }
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
pub fn ColumnarDetails(
    collection: Collection,
    schema: CollectionSchema,
    record: Record,
    access: Option<CollectionAccess>,
) -> Element {
    let mut view = use_context::<Signal<RecordView>>();
    let copier = use_copier();
    let prefs = use_context::<Signal<Preferences>>();
    let offset = prefs.read().utc_offset_minutes;
    let tabs = DetailsTab::for_collection(&collection);
    let active = effective_tab(view.read().details_tab, &tabs).unwrap_or(DetailsTab::Fields);
    let title = schema
        .fields
        .first()
        .and_then(|f| record.get(&f.name).map(text_label))
        .unwrap_or_else(|| record.id.clone());
    let updated = record
        .get("updated_at")
        .map(|v| time_label(v, offset))
        .unwrap_or_default();
    let json = crate::models::record_json::pretty(&record.value, &schema.fields);
    rsx! {
        div { class: "inspector",
            div { class: "inspector-head",
                h3 { class: "inspector-title", "{title}" }
            }
            div { class: "id-line",
                span { class: "mono", "_id: {record.id}" }
                button {
                    class: "icon-btn sm",
                    title: "Copy _id",
                    "aria-label": "Copy _id",
                    onclick: {
                        let id = record.id.clone();
                        move |_| copier.copy(id.clone())
                    },
                    Ico { icon: Icon::Copy, size: 14 }
                }
            }
            DetailTabs { tabs, active }
            match active {
                DetailsTab::Json => rsx! {
                    crate::components::json_view::JsonView { text: json }
                },
                DetailsTab::Fields
                | DetailsTab::Document
                | DetailsTab::Raw
                | DetailsTab::FullText
                | DetailsTab::Vector
                | DetailsTab::Indexes
                | DetailsTab::History
                | DetailsTab::Properties
                | DetailsTab::Geometry => rsx! {
                    div { class: "field-form",
                        for f in schema.fields.iter() {
                            div { key: "{f.name}", class: "form-line",
                                label { r#for: "f-{f.name}", "{f.name}" }
                                div { class: "input-with-hint",
                                    input {
                                        id: "f-{f.name}",
                                        class: "input",
                                        readonly: true,
                                        value: "{value_text(&record, &f.name, offset)}",
                                    }
                                    span { class: "input-hint", "{f.ty.label()}" }
                                }
                            }
                        }
                    }
                },
            }
            div { class: "kv kv-compact",
                div { class: "kv-row",
                    span { "Revision" }
                    span { class: "with-icon", "{record.revision}"
                        button {
                            class: "icon-btn sm",
                            title: "Copy revision",
                            "aria-label": "Copy revision",
                            onclick: {
                                let rev = record.revision.to_string();
                                move |_| copier.copy(rev.clone())
                            },
                            Ico { icon: Icon::Copy, size: 14 }
                        }
                    }
                }
                div { class: "kv-row", span { "Last updated" } span { "{updated}" } }
            }
            div { class: "inspector-actions inline",
                RecordActions {
                    collection: collection.clone(),
                    schema: schema.clone(),
                    record: record.clone(),
                    access,
                    with_copy: false,
                }
                button {
                    class: "btn",
                    onclick: move |_| view.write().details_tab = Some(DetailsTab::Json),
                    Ico { icon: Icon::Braces, size: 16 }
                    "View JSON"
                }
            }
        }
    }
}

fn grant_text(g: &Grant, role: &str) -> String {
    match g {
        Grant::Allowed => format!("Allowed (role: {role})"),
        Grant::Denied(_) => "Not allowed".to_string(),
    }
}

#[component]
pub fn SeriesDetails(
    collection: Collection,
    schema: CollectionSchema,
    record: Record,
    access: Option<CollectionAccess>,
    prev: Option<String>,
    next: Option<String>,
) -> Element {
    let nav = use_navigator();
    let timeseries = use_context::<TimeseriesUi>();
    let prefs = use_context::<Signal<Preferences>>();
    let offset = prefs.read().utc_offset_minutes;
    let zone = offset_label(offset);
    let time_key = schema
        .time_key()
        .map(|f| f.name.clone())
        .unwrap_or_default();
    let order = schema
        .default_sort
        .as_ref()
        .map(|(f, d)| format!("{f} {}", if *d { "desc" } else { "asc" }))
        .unwrap_or_default();
    let when = record
        .get(&time_key)
        .map(|v| time_label(v, offset))
        .unwrap_or_default();
    rsx! {
        div { class: "inspector",
            div { class: "inspector-head split",
                h3 { class: "inspector-title", "Row {record.id}" }
                Stepper { prev, next }
            }
            div { class: "time-key",
                span { class: "field-name", "{time_key}" }
                div { class: "time-box",
                    span { "{when}" }
                    Ico { icon: Icon::Calendar, size: 16 }
                }
                div { class: "time-badges",
                    span { class: "badge blue", "TIME_KEY" }
                    span { class: "meta", "{zone}" }
                }
            }
            div { class: "kv kv-compact",
                for f in schema.fields.iter().filter(|f| !f.time_key) {
                    div { key: "{f.name}", class: "kv-row",
                        span { "{f.name}" }
                        span { "{value_text(&record, &f.name, offset)}" }
                    }
                }
            }
            h4 { class: "inspector-section", "Metadata" }
            div { class: "kv kv-compact",
                div { class: "kv-row", span { "Collection" } span { "{collection.name}" } }
                div { class: "kv-row", span { "Storage type" } span { "{collection.profile.storage_label()}" } }
                div { class: "kv-row", span { "Event time field" } span { "{time_key} ({zone})" } }
                div { class: "kv-row", span { "Order by" } span { "{order}" } }
            }
            h4 { class: "inspector-section", "Permissions" }
            if let Some(a) = access.as_ref() {
                div { class: "kv kv-compact",
                    div { class: "kv-row", span { "Create" } span { "{grant_text(&a.create, &a.role)}" } }
                    div { class: "kv-row", span { "Read" } span { "{grant_text(&a.read, &a.role)}" } }
                    div { class: "kv-row", span { "Update" } span { "{grant_text(&a.update, &a.role)}" } }
                    div { class: "kv-row", span { "Delete" } span { "{grant_text(&a.delete, &a.role)}" } }
                }
            }
            div { class: "inspector-actions",
                button {
                    class: "btn",
                    onclick: move |_| { nav.push(Route::Admin { tab: AdminTab::Access }); },
                    Ico { icon: Icon::Pencil, size: 16 }
                    "Edit permissions"
                }
                button {
                    class: "btn",
                    onclick: {
                        let (name, key) = (collection.name.clone(), time_key.clone());
                        move |_| {
                            timeseries.open_collection(&name, (!key.is_empty()).then_some(key.as_str()));
                            nav.push(Route::Timeseries { tab: TimeseriesTab::Explore });
                        }
                    },
                    Ico { icon: Icon::ChartColumn, size: 16 }
                    "Open in Timeseries"
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::MockConnectionService;
    use crate::services::explorer_data::ExplorerData;

    #[tokio::test]
    async fn structured_values_read_as_a_count() {
        let r = MockConnectionService::ready()
            .record("orders", "o104")
            .await
            .expect("record");
        assert_eq!(value_text(&r, "items", 0), "[ 3 items ]");
        assert_eq!(value_text(&r, "shipping_address", 0), "{ 5 fields }");
        assert_eq!(value_text(&r, "metadata", 0), "{ 2 fields }");
        assert_eq!(value_text(&r, "missing", 0), "–");
    }

    #[test]
    fn a_reference_name_loses_its_plural() {
        assert_eq!(singular("customers"), "customer");
        assert_eq!(singular("data"), "data");
    }

    #[test]
    fn grants_name_the_role_only_when_allowed() {
        assert_eq!(
            grant_text(&Grant::Allowed, "analyst"),
            "Allowed (role: analyst)"
        );
        assert_eq!(
            grant_text(
                &Grant::Denied(crate::models::records::Denial::Privilege),
                "analyst"
            ),
            "Not allowed"
        );
    }
}
