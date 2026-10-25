//! The record table, its empty state, and the page footer.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::components::pager::{Pager, RowsPerPage};
use crate::models::collection::{Collection, Profile};
use crate::models::format::thousands;
use crate::models::records::{Record, RecordPage};
use crate::models::schema::{CellFormat, CollectionSchema, ColumnSpec};
use crate::state::explorer::RecordView;
use crate::state::preferences::Preferences;
use crate::views::explorer::cells::{cell_text, dot_class, status_tone};

fn cell(col: &ColumnSpec, record: &Record, schema: &CollectionSchema, offset: i16) -> Element {
    let text = cell_text(col, record, schema, offset);
    match col.format {
        CellFormat::Id => rsx! { td { class: "key", "{text}" } },
        CellFormat::Number | CellFormat::Bytes | CellFormat::Ttl => {
            rsx! { td { class: "num", "{text}" } }
        }
        CellFormat::StatusDot => {
            let dot = dot_class(status_tone(&text));
            rsx! { td { span { class: "status", span { class: "{dot}" } "{text}" } } }
        }
        CellFormat::StatusBadge => {
            let tone = status_tone(&text);
            rsx! { td { span { class: "badge {tone}", "{text}" } } }
        }
        CellFormat::Embedding | CellFormat::Preview | CellFormat::Geometry => {
            rsx! { td { class: "mono", "{text}" } }
        }
        CellFormat::Text | CellFormat::Timestamp | CellFormat::Srid => rsx! { td { "{text}" } },
    }
}

/// The footer sentence: `1–10 of 104 documents`, never a zero for a
/// collection the filter merely emptied.
fn range_label(
    page: usize,
    size: usize,
    shown: usize,
    total: u64,
    noun: &str,
    narrowed: bool,
) -> String {
    if shown == 0 {
        return if narrowed {
            "No matching records".to_string()
        } else {
            "No records".to_string()
        };
    }
    let from = page * size + 1;
    let to = page * size + shown;
    let filtered = if narrowed { " (filtered)" } else { "" };
    format!("{from}–{to} of {} {noun}{filtered}", thousands(total))
}

#[component]
pub fn RecordTable(
    collection: Collection,
    schema: CollectionSchema,
    page: RecordPage,
    selected: Option<String>,
    filter_open: Signal<bool>,
    /// Whether the user can add records, for the empty state's wording.
    can_insert: bool,
) -> Element {
    let mut view = use_context::<Signal<RecordView>>();
    let prefs = use_context::<Signal<Preferences>>();
    let offset = prefs.read().utc_offset_minutes;
    let v = view.read().clone();
    let size = v.page_size.unwrap_or(schema.default_page_size);
    let sort = v.sort.clone().or_else(|| schema.default_sort.clone());
    let columns: Vec<&ColumnSpec> = schema
        .columns
        .iter()
        .filter(|c| !v.hidden_columns.contains(&c.header))
        .collect();
    let query = v.query(&schema);
    let narrowed = query.narrowed();
    let pages = page.pages(size);
    let label = range_label(
        v.page,
        size,
        page.rows.len(),
        page.total,
        collection.profile.record_noun(),
        narrowed,
    );

    rsx! {
        div { class: "dt-wrap records-wrap",
            table { class: "dt records-table",
                thead {
                    tr {
                        th { class: "check-col",
                            input {
                                r#type: "checkbox",
                                class: "checkbox",
                                disabled: true,
                                title: "Selecting several records is not designed yet",
                                "aria-label": "Select all records",
                            }
                        }
                        for col in columns.iter() {
                            {
                                let field = col.field.clone();
                                let numeric = matches!(col.format, CellFormat::Number | CellFormat::Bytes | CellFormat::Ttl);
                                let sorted = sort.as_ref().filter(|(f, _)| !field.is_empty() && *f == field);
                                let sortable = !field.is_empty()
                                    && !matches!(col.format, CellFormat::Preview | CellFormat::Embedding | CellFormat::Geometry);
                                let ty = schema.field(&col.field).map(|f| f.ty.label());
                                let typed = schema.typed_headers.then_some(ty).flatten();
                                rsx! {
                                    th { key: "{col.header}", class: if numeric { "num" } else { "" },
                                        if sortable {
                                            button {
                                                class: "th-sort",
                                                title: "Sort by {col.header}",
                                                onclick: {
                                                    let current = sorted.map(|(_, d)| *d);
                                                    let field = field.clone();
                                                    move |_| {
                                                        let mut w = view.write();
                                                        w.sort = Some((field.clone(), current == Some(false)));
                                                        w.restart_paging();
                                                    }
                                                },
                                                span { "{col.header}" }
                                                if let Some((_, desc)) = sorted {
                                                    Ico { icon: if *desc { Icon::ArrowDown } else { Icon::ArrowUp }, size: 14 }
                                                }
                                            }
                                        } else {
                                            span { "{col.header}" }
                                        }
                                        if let Some(t) = typed {
                                            div { class: "th-type", "{t}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                tbody {
                    for record in page.rows.iter() {
                        {
                            let id = record.id.clone();
                            let is_selected = selected.as_deref() == Some(record.id.as_str());
                            let pick = {
                                let id = id.clone();
                                move |_| view.write().record = Some(id.clone())
                            };
                            rsx! {
                                tr {
                                    key: "{record.id}",
                                    class: if is_selected { "clickable selected" } else { "clickable" },
                                    "aria-selected": "{is_selected}",
                                    onclick: pick.clone(),
                                    td { class: "check-col",
                                        input {
                                            r#type: "checkbox",
                                            class: "checkbox",
                                            checked: is_selected,
                                            "aria-label": "Select {record.id}",
                                            onclick: move |e| e.stop_propagation(),
                                            onchange: move |_| view.write().record = Some(id.clone()),
                                        }
                                    }
                                    for col in columns.iter() {
                                        {cell(col, record, &schema, offset)}
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if page.rows.is_empty() {
                EmptyRecords {
                    collection: collection.name.clone(),
                    profile: collection.profile,
                    narrowed,
                    range_label: schema.time_key().map(|_| query.range.label().to_string()),
                    filter_label: v.filter.as_ref().map(|f| f.label()),
                    can_insert,
                    filter_open,
                }
            }
        }
        div { class: "dt-footer records-footer",
            span { class: "records-count", "{label}" }
            if pages > 1 {
                Pager {
                    page: v.page,
                    pages,
                    on_page: move |p| {
                        let mut w = view.write();
                        w.page = p;
                        w.record = None;
                    },
                }
            }
            div { class: "toolbar-spacer" }
            RowsPerPage {
                value: size,
                on_change: move |n| {
                    let mut w = view.write();
                    w.page_size = Some(n);
                    w.restart_paging();
                },
            }
        }
    }
}

/// The body of an empty page. A filter that matched nothing says so and
/// offers the way out. A collection that holds nothing says that instead.
#[component]
fn EmptyRecords(
    collection: String,
    profile: Profile,
    narrowed: bool,
    range_label: Option<String>,
    filter_label: Option<String>,
    can_insert: bool,
    filter_open: Signal<bool>,
) -> Element {
    let mut view = use_context::<Signal<RecordView>>();
    let mut filter_open = filter_open;
    let noun = profile.record_noun();
    rsx! {
        div { class: "empty-state records-empty",
            if narrowed {
                Ico { icon: Icon::Filter, size: 32 }
                h3 { "No records match your filter" }
                p {
                    if let Some(f) = &filter_label {
                        "No {noun} were found with {f}."
                    } else {
                        "No {noun} in {collection} match the current filters."
                    }
                }
                div { class: "actions",
                    button {
                        class: "btn",
                        onclick: move |_| {
                            let mut w = view.write();
                            w.filter = None;
                            w.text.clear();
                            w.bbox = None;
                            w.bbox_text.clear();
                            w.restart_paging();
                        },
                        "Clear filter"
                    }
                    if filter_label.is_some() {
                        button { class: "btn primary", onclick: move |_| filter_open.set(true), "Edit filter" }
                    }
                }
            } else if let Some(range) = &range_label {
                Ico { icon: Icon::Clock, size: 32 }
                h3 { "No rows in this time range" }
                p { "{collection} has no rows for {range}. Choose a longer range." }
            } else {
                Ico { icon: Icon::FileText, size: 32 }
                h3 { "{collection} holds no records" }
                p {
                    if can_insert {
                        "Insert a record to start."
                    } else {
                        "Your session cannot insert records."
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_range_reads_one_based_and_names_the_noun() {
        assert_eq!(
            range_label(0, 10, 10, 104, "documents", false),
            "1–10 of 104 documents"
        );
        assert_eq!(
            range_label(1, 5, 5, 1284, "documents", false),
            "6–10 of 1,284 documents"
        );
        assert_eq!(
            range_label(0, 50, 5, 5, "rows", true),
            "1–5 of 5 rows (filtered)"
        );
    }

    #[test]
    fn an_emptied_filter_never_reports_a_collection_zero() {
        assert_eq!(
            range_label(0, 50, 0, 0, "rows", true),
            "No matching records"
        );
        assert_eq!(range_label(0, 50, 0, 0, "rows", false), "No records");
    }
}
