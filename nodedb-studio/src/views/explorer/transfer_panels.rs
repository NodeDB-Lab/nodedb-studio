//! Data Transfer page pieces: the page tabs and the Export and Import tab
//! bodies.

use dioxus::prelude::*;

use crate::components::async_view::AsyncView;
use crate::components::icon::{Ico, Icon};
use crate::models::records::Filter;
use crate::models::schema::CollectionSchema;
use crate::models::transfer::{Delimiter, ExportFormat, ExportPreview, ExportRequest};
use crate::routes::Route;
use crate::services::async_state::AsyncState;
use crate::state::explorer::{CollectionTab, RecordView};
use crate::views::explorer::transfer_form::{ExportForm, summary};

/// The page tabs: three views of the collection, then Data Transfer.
#[component]
pub(super) fn PageTabs() -> Element {
    let nav = use_navigator();
    let mut view = use_context::<Signal<RecordView>>();
    let mut go = move |tab: CollectionTab| {
        view.write().tab = tab;
        nav.push(Route::Explorer {});
    };
    rsx! {
        nav { class: "page-tabs", role: "tablist",
            for tab in [CollectionTab::Data, CollectionTab::Schema, CollectionTab::Indexes] {
                button {
                    key: "{tab.label()}",
                    class: "tab",
                    role: "tab",
                    "aria-selected": "false",
                    onclick: move |_| go(tab),
                    "{tab.label()}"
                }
            }
            button { class: "tab active", role: "tab", "aria-selected": "true", "Data Transfer" }
            button {
                class: "tab",
                role: "tab",
                "aria-selected": "false",
                disabled: true,
                title: "Collection settings are not designed yet",
                "Settings"
            }
        }
    }
}

/// The Export tab: format, scope, fields, delimiter, preview, and the start
/// button.
#[component]
pub(super) fn ExportPane(
    form: Signal<ExportForm>,
    schema: Option<CollectionSchema>,
    filter: Option<Filter>,
    request: Option<ExportRequest>,
    preview: AsyncState<ExportPreview>,
    on_start: EventHandler<()>,
) -> Element {
    let mut form = form;
    let f = form.read().clone();
    let scope_filtered = filter.is_some() && f.filtered;
    let fields = schema.as_ref().map(|s| f.fields(s)).unwrap_or_default();
    let addable: Vec<String> = schema
        .as_ref()
        .map(|s| {
            s.fields
                .iter()
                .map(|x| x.name.clone())
                .filter(|n| !fields.contains(n))
                .collect()
        })
        .unwrap_or_default();
    rsx! {
        div { class: "transfer-form",
            div { class: "form-line",
                span { class: "form-label", "Format" }
                div { class: "radio-row", role: "radiogroup", "aria-label": "Format",
                    for fmt in ExportFormat::ALL {
                        label { key: "{fmt.label()}", class: "check-row",
                            input {
                                r#type: "radio",
                                class: "radio",
                                name: "format",
                                checked: f.format == fmt,
                                onchange: move |_| form.write().format = fmt,
                            }
                            "{fmt.label()}"
                        }
                    }
                }
            }
            div { class: "form-line",
                label { class: "form-label", r#for: "scope", "Scope" }
                div { class: "scope-row",
                    select {
                        id: "scope",
                        class: "select native",
                        onchange: move |e| form.write().filtered = e.value() == "filtered",
                        option { value: "all", selected: !scope_filtered, "All rows" }
                        option { value: "filtered", selected: scope_filtered, disabled: filter.is_none(), "Filtered rows" }
                    }
                    if let Some(flt) = filter.clone() {
                        if scope_filtered {
                            span { class: "status active-filter",
                                span { class: "status-dot ok" }
                                "Active filter · {flt.label()}"
                            }
                        }
                        crate::views::explorer::links::OpenLink {
                            route: Route::Explorer {},
                            label: "Edit filter".to_string(),
                            icon: Icon::Filter,
                            class: "btn".to_string(),
                        }
                    } else {
                        span { class: "meta", "Set a filter in Explorer to export only matching rows." }
                    }
                }
            }
            div { class: "form-line top",
                span { class: "form-label", "Selected fields" }
                div { class: "field-chips",
                    for fld in fields.iter() {
                        {
                            let fld = fld.clone();
                            rsx! {
                                span { key: "{fld}", class: "chip",
                                    "{fld}"
                                    button {
                                        class: "chip-remove",
                                        title: "Remove {fld}",
                                        "aria-label": "Remove field {fld}",
                                        onclick: {
                                            let fld = fld.clone();
                                            let all = fields.clone();
                                            move |_| form.write().fields = Some(all.iter().filter(|x| **x != fld).cloned().collect())
                                        },
                                        Ico { icon: Icon::X, size: 14 }
                                    }
                                }
                            }
                        }
                    }
                    if !addable.is_empty() {
                        select {
                            class: "select native compact add-field",
                            "aria-label": "Add field",
                            onchange: {
                                let all = fields.clone();
                                move |e| {
                                    let v = e.value();
                                    if !v.is_empty() {
                                        let mut next = all.clone();
                                        next.push(v);
                                        form.write().fields = Some(next);
                                    }
                                }
                            },
                            option { value: "", selected: true, "Add field…" }
                            for a in addable.iter() {
                                option { key: "{a}", value: "{a}", "{a}" }
                            }
                        }
                    }
                }
            }
            if f.format.delimited() {
                div { class: "form-line",
                    span { class: "form-label" }
                    label { class: "check-row",
                        input {
                            r#type: "checkbox",
                            class: "checkbox",
                            checked: f.header,
                            onchange: move |e| form.write().header = e.checked(),
                        }
                        "Include header row"
                    }
                }
                div { class: "form-line",
                    label { class: "form-label", r#for: "encoding", "Encoding" }
                    select { id: "encoding", class: "select native", disabled: true, title: "UTF-8 is the only encoding an export writes",
                        option { "UTF-8" }
                    }
                }
                div { class: "form-line",
                    label { class: "form-label", r#for: "delimiter", "Delimiter" }
                    select {
                        id: "delimiter",
                        class: "select native",
                        onchange: move |e| {
                            if let Some(d) = Delimiter::ALL.iter().find(|d| d.label() == e.value()) {
                                form.write().delimiter = *d;
                            }
                        },
                        for d in Delimiter::ALL {
                            option { key: "{d.label()}", value: "{d.label()}", selected: d == f.delimiter, "{d.label()}" }
                        }
                    }
                }
            }
            div { class: "preview-head",
                h3 { class: "section-title", "Preview (first 3 rows)" }
                crate::views::explorer::links::OpenLink {
                    route: Route::Explorer {},
                    label: "View more rows".to_string(),
                    icon: Icon::ExternalLink,
                    class: "btn".to_string(),
                }
            }
            AsyncView {
                loading: preview.is_loading(),
                empty: false,
                error: preview.error_message(),
                retriable: preview.is_retriable(),
            }
            if let Some(p) = preview.loaded().cloned() {
                div { class: "dt-wrap preview-table",
                    table { class: "dt",
                        thead { tr { for col in p.columns.iter() { th { key: "{col}", "{col}" } } } }
                        tbody {
                            for (i, row) in p.rows.iter().enumerate() {
                                tr { key: "{i}", for (j, cell) in row.iter().enumerate() { td { key: "{j}", "{cell}" } } }
                            }
                        }
                    }
                }
                div { class: "export-bar",
                    button {
                        class: "btn primary",
                        disabled: fields.is_empty(),
                        title: if fields.is_empty() { "Select at least one field" } else { "" },
                        onclick: move |_| on_start.call(()),
                        Ico { icon: Icon::Download, size: 16 }
                        "Export to {f.format.label()}"
                    }
                    if let Some(r) = request.as_ref() {
                        span { class: "meta", "{summary(r, p.total)}" }
                    }
                }
                p { class: "meta secrets-note", "Credentials and secrets are never exported." }
            }
        }
    }
}

/// The Import tab: the disabled file picker and the add-only notice for the
/// collection `name`.
#[component]
pub(super) fn ImportPane(name: String) -> Element {
    rsx! {
        div { class: "transfer-form",
            div { class: "callout info",
                Ico { icon: Icon::Info, size: 16 }
                span { "An import adds records to {name}. It never overwrites a stored record: a row whose id already exists is rejected." }
            }
            div { class: "form-line",
                label { class: "form-label", r#for: "import-file", "File" }
                div { class: "scope-row",
                    input { id: "import-file", class: "input mono", readonly: true, placeholder: "No file selected" }
                    button { class: "btn", disabled: true, title: "The file picker is not available yet", "Browse" }
                }
            }
            div { class: "form-line",
                span { class: "form-label", "Target" }
                span { "{name}" }
            }
            div { class: "form-line",
                span { class: "form-label" }
                button {
                    class: "link-btn",
                    disabled: true,
                    title: "Import mapping and validation are not designed yet",
                    "Open mapping and validation"
                }
            }
            div { class: "export-bar",
                button { class: "btn primary", disabled: true, title: "Choose a file first",
                    Ico { icon: Icon::Upload, size: 16 }
                    "Start import"
                }
                span { class: "meta", "Credentials and secrets are never imported." }
            }
        }
    }
}
