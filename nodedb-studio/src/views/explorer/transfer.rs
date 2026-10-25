//! Data Transfer: export a collection, import into it, and watch the jobs.
//!
//! An export reads the rows the Explorer's filter matches, or the whole
//! collection. It never includes credentials or secrets. An import adds
//! records and never overwrites a stored one. A job runs on the server:
//! Studio starts it, lists it, and cancels it.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::components::tabs::TabBar;
use crate::models::collection::{Collection, Profile};
use crate::models::format::thousands;
use crate::models::records::Filter;
use crate::models::schema::CollectionSchema;
use crate::models::transfer::{Delimiter, ExportFormat, ExportRequest, ExportScope};
use crate::routes::Route;
use crate::services::backend::Backend;
use crate::state::explorer::{
    CollectionTab, RecordView, RecordsEpoch, Selected, TransferNotice, TransferTab,
    select_collection,
};
use crate::state::preferences::Preferences;
use crate::state::workspace::Surface;
use crate::views::explorer::transfer_jobs::{HistoryTable, JobsTable, recent};

const SURFACE: Surface = Surface::DataTransfer;
const JOBS_SIZE: PanelSize = PanelSize::width(560.0, 380.0, 760.0);

/// What the export form holds.
#[derive(Debug, Clone, PartialEq)]
struct ExportForm {
    format: ExportFormat,
    /// Read only the rows the Explorer's filter matches. Ignored without one.
    filtered: bool,
    /// `None` exports the collection's first four fields.
    fields: Option<Vec<String>>,
    header: bool,
    delimiter: Delimiter,
}

impl Default for ExportForm {
    fn default() -> Self {
        Self {
            format: ExportFormat::Csv,
            filtered: true,
            fields: None,
            header: true,
            delimiter: Delimiter::Comma,
        }
    }
}

impl ExportForm {
    /// The fields to export: the chosen ones, else the first four.
    fn fields(&self, schema: &CollectionSchema) -> Vec<String> {
        self.fields.clone().unwrap_or_else(|| {
            schema
                .fields
                .iter()
                .take(4)
                .map(|f| f.name.clone())
                .collect()
        })
    }

    fn request(
        &self,
        collection: &str,
        schema: &CollectionSchema,
        filter: Option<&Filter>,
    ) -> ExportRequest {
        ExportRequest {
            collection: collection.to_string(),
            format: self.format,
            scope: match filter {
                Some(f) if self.filtered => ExportScope::Filtered(f.clone()),
                Some(_) | None => ExportScope::All,
            },
            fields: self.fields(schema),
            header: self.header,
            delimiter: self.delimiter,
        }
    }
}

/// `Exports 842 filtered rows with 4 fields (CSV, UTF-8, comma)`.
fn summary(request: &ExportRequest, total: u64) -> String {
    let which = match request.scope {
        ExportScope::Filtered(_) => "filtered rows",
        ExportScope::All => "rows",
    };
    let detail = if request.format.delimited() {
        format!(
            "{}, UTF-8, {}",
            request.format.label(),
            request.delimiter.word()
        )
    } else {
        request.format.label().to_string()
    };
    format!(
        "Exports {} {which} with {} fields ({detail})",
        thousands(total),
        request.fields.len()
    )
}

/// The page tabs: three views of the collection, then Data Transfer.
#[component]
fn PageTabs() -> Element {
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

#[component]
pub fn DataTransferPage() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let epoch = use_context::<RecordsEpoch>();
    let mut selected = use_context::<Signal<Option<Selected>>>();
    let mut view = use_context::<Signal<RecordView>>();
    let notice = use_context::<TransferNotice>();
    let prefs = use_context::<Signal<Preferences>>();
    let mut form = use_signal(ExportForm::default);
    let offset = prefs.read().utc_offset_minutes;

    let catalog = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let _conn = conn();
            async move { backend.catalog().await }
        })
    };
    let catalog_state = resource_state(&catalog);
    let collections: Vec<Collection> = catalog_state
        .loaded()
        .map(|all| {
            all.iter()
                .filter(|c| c.profile != Profile::Array)
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    // The Explorer's collection, or the first one when none is chosen. A
    // memo, so the reads below follow the catalog when it loads.
    let name = use_memo(move || {
        let list: Vec<String> = catalog
            .read()
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|all| {
                all.iter()
                    .filter(|c| c.profile != Profile::Array)
                    .map(|c| c.name.clone())
                    .collect()
            })
            .unwrap_or_default();
        let chosen = selected.read().as_ref().map(|s| s.name.clone());
        chosen
            .filter(|n| list.contains(n))
            .or_else(|| list.first().cloned())
    });
    let current: Option<Collection> = name
        .read()
        .as_ref()
        .and_then(|n| collections.iter().find(|c| &c.name == n).cloned());

    let schema = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let name = name();
            let _conn = conn();
            async move {
                match name {
                    Some(n) => backend.collection_schema(&n).await,
                    None => std::future::pending().await,
                }
            }
        })
    };
    let schema_state = resource_state(&schema);

    let filter = view.read().filter.clone();
    let request = use_memo(move || {
        let s = schema
            .read()
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .cloned()?;
        let n = s.collection.clone();
        let f = view.read().filter.clone();
        Some(form.read().request(&n, &s, f.as_ref()))
    });
    let preview = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let req = request();
            let _epoch = epoch.0.read();
            let _conn = conn();
            async move {
                match req {
                    Some(r) => backend.export_preview(&r).await,
                    None => std::future::pending().await,
                }
            }
        })
    };
    let preview_state = resource_state(&preview);
    let jobs = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let _epoch = epoch.0.read();
            let _conn = conn();
            async move { backend.transfer_jobs().await }
        })
    };
    let jobs_state = resource_state(&jobs);

    let start = {
        let backend = backend.clone();
        move |_| {
            let Some(req) = request.peek().clone() else {
                return;
            };
            let backend = backend.clone();
            let (mut epoch, mut notice) = (epoch.0, notice.0);
            notice.set(None);
            spawn_forever(async move {
                match backend.start_export(&req).await {
                    Ok(_) => {}
                    Err(e) => notice.set(Some(e.to_string())),
                }
                let next = *epoch.peek() + 1;
                epoch.set(next);
            });
        }
    };
    let cancel = {
        let backend = backend.clone();
        move |id: String| {
            let backend = backend.clone();
            let (mut epoch, mut notice) = (epoch.0, notice.0);
            notice.set(None);
            spawn_forever(async move {
                if let Err(e) = backend.cancel_job(&id).await {
                    notice.set(Some(e.to_string()));
                }
                let next = *epoch.peek() + 1;
                epoch.set(next);
            });
        }
    };

    let f = form.read().clone();
    let scope_filtered = filter.is_some() && f.filtered;
    let all_jobs = jobs_state.loaded().cloned().unwrap_or_default();
    let background: Vec<_> = recent(&all_jobs).into_iter().cloned().collect();
    let current_schema = schema_state.loaded().cloned();
    let fields = current_schema
        .as_ref()
        .map(|s| f.fields(s))
        .unwrap_or_default();
    let addable: Vec<String> = current_schema
        .as_ref()
        .map(|s| {
            s.fields
                .iter()
                .map(|x| x.name.clone())
                .filter(|n| !fields.contains(n))
                .collect()
        })
        .unwrap_or_default();
    let tab_index = TransferTab::ALL
        .iter()
        .position(|t| *t == view.read().transfer_tab)
        .unwrap_or(1);

    rsx! {
        div { class: "page-header transfer-header",
            Ico { icon: Icon::Folder, size: 24 }
            h1 { class: "page-title-inline", "Explorer" }
            if !collections.is_empty() {
                label { class: "select-with-icon",
                    Ico { icon: Icon::Table, size: 16 }
                    select {
                        class: "select native",
                        "aria-label": "Collection",
                        onchange: move |e| {
                            select_collection(&mut selected, &mut view, &e.value());
                            form.set(ExportForm::default());
                        },
                        for c in collections.iter() {
                            option {
                                key: "{c.name}",
                                value: "{c.name}",
                                selected: Some(&c.name) == current.as_ref().map(|c| &c.name),
                                "{c.name}"
                            }
                        }
                    }
                }
            }
        }
        PageTabs {}
        div { class: "page-body",
            div { class: "workbench",
                Panel {
                    surface: SURFACE,
                    id: "transfer",
                    title: if view.read().transfer_tab == TransferTab::Import { "Import data" } else if view.read().transfer_tab == TransferTab::History { "Transfer history" } else { "Export data" },
                    class: "fill",
                    TabBar {
                        items: TransferTab::ALL.iter().map(|t| t.label()).collect(),
                        active: tab_index,
                        class: "collection-tabs",
                        on_select: move |i: usize| {
                            if let Some(t) = TransferTab::ALL.get(i) {
                                view.write().transfer_tab = *t;
                            }
                        },
                    }
                    AsyncView {
                        loading: catalog_state.is_loading(),
                        empty: catalog_state.is_empty(),
                        error: catalog_state.error_message(),
                        retriable: catalog_state.is_retriable(),
                        empty_message: "This database has no collections to transfer.",
                    }
                    if let Some(msg) = notice.0.read().clone() {
                        div { class: "callout err transfer-notice", role: "alert",
                            Ico { icon: Icon::CircleAlert, size: 16 }
                            span { "{msg}" }
                        }
                    }
                    if let Some(c) = current.clone() {
                        match view.read().transfer_tab {
                            TransferTab::Export => rsx! {
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
                                        loading: preview_state.is_loading(),
                                        empty: false,
                                        error: preview_state.error_message(),
                                        retriable: preview_state.is_retriable(),
                                    }
                                    if let Some(p) = preview_state.loaded().cloned() {
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
                                                onclick: start,
                                                Ico { icon: Icon::Download, size: 16 }
                                                "Export to {f.format.label()}"
                                            }
                                            if let Some(r) = request.read().as_ref() {
                                                span { class: "meta", "{summary(r, p.total)}" }
                                            }
                                        }
                                        p { class: "meta secrets-note", "Credentials and secrets are never exported." }
                                    }
                                }
                            },
                            TransferTab::Import => rsx! {
                                div { class: "transfer-form",
                                    div { class: "callout info",
                                        Ico { icon: Icon::Info, size: 16 }
                                        span { "An import adds records to {c.name}. It never overwrites a stored record: a row whose id already exists is rejected." }
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
                                        span { "{c.name}" }
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
                            },
                            TransferTab::History => rsx! {
                                HistoryTable { jobs: all_jobs.clone(), offset }
                            },
                        }
                    }
                }
                ResizeHandle { surface: SURFACE, target: "jobs", size: JOBS_SIZE, side: Side::After }
                Panel { surface: SURFACE, id: "jobs", title: "Background jobs", size: JOBS_SIZE,
                    AsyncView {
                        loading: jobs_state.is_loading(),
                        empty: jobs_state.is_empty(),
                        error: jobs_state.error_message(),
                        retriable: jobs_state.is_retriable(),
                        empty_message: "No transfers have run on this database.",
                    }
                    if !background.is_empty() {
                        JobsTable { jobs: background, offset, on_cancel: cancel }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::transfer::ExportFormat;
    use crate::services::MockConnectionService;
    use crate::services::explorer_data::ExplorerData;

    fn active() -> Filter {
        Filter {
            field: "status".into(),
            value: "active".into(),
        }
    }

    #[tokio::test]
    async fn the_default_fields_are_the_first_four() {
        let schema = MockConnectionService::ready()
            .collection_schema("users")
            .await
            .expect("schema");
        let form = ExportForm::default();
        assert_eq!(form.fields(&schema), ["id", "name", "email", "status"]);
        let request = form.request("users", &schema, Some(&active()));
        assert_eq!(request.scope, ExportScope::Filtered(active()));
    }

    #[tokio::test]
    async fn without_a_filter_the_scope_is_every_row() {
        let schema = MockConnectionService::ready()
            .collection_schema("users")
            .await
            .expect("schema");
        let request = ExportForm::default().request("users", &schema, None);
        assert_eq!(request.scope, ExportScope::All);
        let unfiltered = ExportForm {
            filtered: false,
            ..ExportForm::default()
        };
        assert_eq!(
            unfiltered.request("users", &schema, Some(&active())).scope,
            ExportScope::All
        );
    }

    #[tokio::test]
    async fn the_summary_states_scope_fields_and_format() {
        let schema = MockConnectionService::ready()
            .collection_schema("users")
            .await
            .expect("schema");
        let form = ExportForm::default();
        let r = form.request("users", &schema, Some(&active()));
        assert_eq!(
            summary(&r, 842),
            "Exports 842 filtered rows with 4 fields (CSV, UTF-8, comma)"
        );
        let json = ExportForm {
            format: ExportFormat::Jsonl,
            ..ExportForm::default()
        }
        .request("users", &schema, None);
        assert_eq!(
            summary(&json, 1284),
            "Exports 1,284 rows with 4 fields (JSONL)"
        );
    }
}
