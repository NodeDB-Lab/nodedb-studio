//! The controls above a record table: filter, sort, columns, time range,
//! bounding box, text search, Export, and the write buttons.
//!
//! Which controls a collection gets follows its profile and capabilities.
//! Every control writes `RecordView`, never a local copy, so the Data
//! Transfer page and a return to the Explorer see the same view.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::collection::{Attached, Collection, Profile};
use crate::models::records::{Bbox, CollectionAccess, Filter, Grant, Record, TimeRange};
use crate::models::schema::{CellFormat, CollectionSchema};
use crate::routes::Route;
use crate::state::explorer::RecordView;
use crate::state::workspace::{Surface, Workspace};
use crate::views::explorer::actions::use_launcher;

/// Which controls one collection shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Controls {
    pub filter: bool,
    pub sort: bool,
    pub text: Option<&'static str>,
    pub range: bool,
    pub bbox: bool,
}

pub fn controls_for(c: &Collection) -> Controls {
    let none = Controls {
        filter: false,
        sort: false,
        text: None,
        range: false,
        bbox: false,
    };
    match c.profile {
        Profile::Document if c.has(Attached::FullText) || c.has(Attached::Vector) => Controls {
            text: Some("Filter documents…"),
            ..none
        },
        Profile::Document => Controls {
            filter: true,
            sort: true,
            ..none
        },
        Profile::DocumentStrict | Profile::Columnar => Controls {
            filter: true,
            sort: true,
            ..none
        },
        Profile::KeyValue => Controls {
            text: Some("Filter keys…"),
            ..none
        },
        Profile::Timeseries => Controls {
            range: true,
            text: Some("Filter rows…"),
            ..none
        },
        Profile::Spatial => Controls { bbox: true, ..none },
        Profile::Array => none,
    }
}

/// `field:desc`, the value a sort option carries.
fn sort_key(field: &str, descending: bool) -> String {
    format!("{field}:{}", if descending { "desc" } else { "asc" })
}

/// The fields a table can sort by: columns that read a scalar field.
pub fn sortable(schema: &CollectionSchema) -> Vec<String> {
    schema
        .columns
        .iter()
        .filter(|c| {
            !c.field.is_empty()
                && !matches!(
                    c.format,
                    CellFormat::Preview
                        | CellFormat::Embedding
                        | CellFormat::Geometry
                        | CellFormat::Srid
                )
        })
        .map(|c| c.field.clone())
        .collect()
}

#[component]
pub fn FilterControl(schema: CollectionSchema, open: Signal<bool>) -> Element {
    let mut view = use_context::<Signal<RecordView>>();
    let mut open = open;
    let current = view.read().filter.clone();
    let first = use_signal(|| schema.filter_fields.first().cloned().unwrap_or_default());
    let mut field = use_signal(|| first.peek().clone());
    let mut value = use_signal(String::new);

    let mut edit = move || {
        let now = view.peek().filter.clone();
        field.set(
            now.as_ref()
                .map_or_else(|| first.peek().clone(), |f| f.field.clone()),
        );
        value.set(now.map(|f| f.value).unwrap_or_default());
        open.set(!open());
    };
    let mut commit = move || {
        let (f, v) = (field.peek().clone(), value.peek().trim().to_string());
        let mut w = view.write();
        w.filter = if v.is_empty() {
            None
        } else {
            Some(Filter { field: f, value: v })
        };
        w.restart_paging();
        open.set(false);
    };
    let clear = move |_| {
        let mut w = view.write();
        w.filter = None;
        w.restart_paging();
        open.set(false);
    };

    rsx! {
        div { class: "anchor",
            button {
                class: if open() || current.is_some() { "icon-btn bordered pressed" } else { "icon-btn bordered" },
                title: "Filter",
                "aria-label": "Filter",
                "aria-expanded": "{open()}",
                onclick: move |e| { e.stop_propagation(); edit(); },
                Ico { icon: Icon::Filter, size: 16 }
            }
            if open() {
                div {
                    class: "popover align-left filter-popover",
                    role: "dialog",
                    "aria-label": "Edit filter",
                    onclick: move |e| e.stop_propagation(),
                    onkeydown: move |e| { if e.key() == Key::Escape { open.set(false); } },
                    div { class: "field",
                        label { r#for: "filter-field", "Field" }
                        select {
                            id: "filter-field",
                            class: "select native",
                            onchange: move |e| field.set(e.value()),
                            for f in schema.filter_fields.iter() {
                                option { key: "{f}", value: "{f}", selected: *f == *field.read(), "{f}" }
                            }
                        }
                    }
                    div { class: "field",
                        label { r#for: "filter-value", "Equals" }
                        input {
                            id: "filter-value",
                            class: "input",
                            value: "{value}",
                            autofocus: true,
                            placeholder: "Value",
                            oninput: move |e| value.set(e.value()),
                            onkeydown: move |e| { if e.key() == Key::Enter { commit(); } },
                        }
                    }
                    div { class: "popover-actions",
                        button { class: "btn sm", onclick: clear, "Clear" }
                        button { class: "btn sm primary", onclick: move |_| commit(), "Apply" }
                    }
                }
            }
        }
        if let Some(f) = current.clone() {
            span { class: "chip filter-chip",
                button {
                    class: "chip-label",
                    title: "Edit filter",
                    onclick: move |e| { e.stop_propagation(); edit(); },
                    "{f.label()}"
                }
                button {
                    class: "chip-remove",
                    title: "Remove filter",
                    "aria-label": "Remove filter {f.label()}",
                    onclick: clear,
                    Ico { icon: Icon::X, size: 14 }
                }
            }
        }
    }
}

#[component]
pub fn SortControl(schema: CollectionSchema) -> Element {
    let mut view = use_context::<Signal<RecordView>>();
    let effective = view
        .read()
        .sort
        .clone()
        .or_else(|| schema.default_sort.clone());
    let current = effective.map(|(f, d)| sort_key(&f, d)).unwrap_or_default();
    let fields = sortable(&schema);
    rsx! {
        label { class: "toolbar-select",
            Ico { icon: Icon::ArrowUpDown, size: 16 }
            select {
                class: "select native compact",
                "aria-label": "Sort",
                onchange: move |e| {
                    let v = e.value();
                    if let Some((field, dir)) = v.split_once(':') {
                        let mut w = view.write();
                        w.sort = Some((field.to_string(), dir == "desc"));
                        w.restart_paging();
                    }
                },
                for f in fields {
                    for desc in [false, true] {
                        {
                            let key = sort_key(&f, desc);
                            let dir = if desc { "desc" } else { "asc" };
                            rsx! {
                                option {
                                    key: "{key}",
                                    value: "{key}",
                                    selected: key == current,
                                    "{f} {dir}"
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
pub fn ColumnsControl(schema: CollectionSchema) -> Element {
    let mut view = use_context::<Signal<RecordView>>();
    let mut open = use_signal(|| false);
    let hidden = view.read().hidden_columns.clone();
    let shown = schema.columns.len() - hidden.len().min(schema.columns.len());
    let label = if hidden.is_empty() {
        "Columns".to_string()
    } else {
        format!("Columns ({shown})")
    };
    rsx! {
        div { class: "anchor",
            button {
                class: "icon-btn bordered",
                title: "{label}",
                "aria-label": "{label}",
                "aria-haspopup": "dialog",
                "aria-expanded": "{open()}",
                onclick: move |e| { e.stop_propagation(); open.set(!open()); },
                Ico { icon: Icon::Columns3, size: 16 }
            }
            if open() {
                div {
                    class: "popover align-left columns-popover",
                    role: "dialog",
                    "aria-label": "Visible columns",
                    onclick: move |e| e.stop_propagation(),
                    onkeydown: move |e| { if e.key() == Key::Escape { open.set(false); } },
                    for c in schema.columns.iter() {
                        {
                            let header = c.header.clone();
                            let on = !hidden.contains(&header);
                            // The last visible column cannot go: a table needs one.
                            let locked = on && shown <= 1;
                            rsx! {
                                label { key: "{c.header}", class: "menu-check", title: if locked { "A table keeps at least one column" } else { "" },
                                    input {
                                        r#type: "checkbox",
                                        class: "checkbox",
                                        checked: on,
                                        disabled: locked,
                                        onchange: move |e| {
                                            let mut w = view.write();
                                            if e.checked() { w.hidden_columns.remove(&header); } else { w.hidden_columns.insert(header.clone()); }
                                        },
                                    }
                                    span { class: "menu-check-text", span { class: "menu-check-label", "{c.header}" } }
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
pub fn TimeRangeControl() -> Element {
    let mut view = use_context::<Signal<RecordView>>();
    let current = view.read().range;
    rsx! {
        label { class: "toolbar-select",
            Ico { icon: Icon::Calendar, size: 16 }
            span { "Time range" }
            select {
                class: "select native compact",
                "aria-label": "Time range",
                onchange: move |e| {
                    if let Some(r) = TimeRange::ALL.iter().find(|r| r.label() == e.value()) {
                        let mut w = view.write();
                        w.range = *r;
                        w.restart_paging();
                    }
                },
                for r in TimeRange::ALL {
                    option { key: "{r.label()}", value: "{r.label()}", selected: r == current, "{r.label()}" }
                }
            }
        }
    }
}

#[component]
pub fn BboxControl() -> Element {
    let mut view = use_context::<Signal<RecordView>>();
    let text = view.read().bbox_text.clone();
    let error = if text.trim().is_empty() {
        None
    } else {
        Bbox::parse(&text).err()
    };
    rsx! {
        div { class: "bbox-field",
            div { class: "search-field",
                Ico { icon: Icon::Map, size: 16 }
                input {
                    class: if error.is_some() { "input invalid" } else { "input" },
                    placeholder: "Filter bbox: min lon, min lat, max lon, max lat",
                    "aria-label": "Filter by bounding box",
                    value: "{text}",
                    oninput: move |e| {
                        let t = e.value();
                        let mut w = view.write();
                        w.bbox = Bbox::parse(&t).ok();
                        w.bbox_text = t;
                        w.restart_paging();
                    },
                }
            }
            if let Some(e) = &error {
                div { class: "field-error", role: "alert", "{e}" }
            }
        }
    }
}

#[component]
pub fn TextControl(placeholder: &'static str) -> Element {
    let mut view = use_context::<Signal<RecordView>>();
    rsx! {
        div { class: "search-field grow",
            Ico { icon: Icon::Search, size: 16 }
            input {
                class: "input",
                placeholder,
                "aria-label": placeholder,
                value: "{view.read().text}",
                oninput: move |e| {
                    let mut w = view.write();
                    w.text = e.value();
                    w.restart_paging();
                },
            }
        }
    }
}

/// The write rights the toolbar acts on. `None` while they load.
fn grant(
    access: &Option<CollectionAccess>,
    pick: fn(&CollectionAccess) -> &Grant,
) -> Option<Grant> {
    access.as_ref().map(|a| pick(a).clone())
}

#[component]
pub fn RecordsToolbar(
    collection: Collection,
    schema: CollectionSchema,
    access: Option<CollectionAccess>,
    /// The record Edit and Delete act on, when the details panel is hidden.
    selected: Option<Record>,
    filter_open: Signal<bool>,
) -> Element {
    let nav = use_navigator();
    let launcher = use_launcher();
    let workspace = use_context::<Signal<Workspace>>();
    let details_visible = workspace.read().is_visible(Surface::Explorer, "details");
    let spec = controls_for(&collection);
    let create = grant(&access, |a| &a.create);
    let update = grant(&access, |a| &a.update);
    let delete = grant(&access, |a| &a.delete);
    let denied_create = create.as_ref().is_some_and(|g| !g.allowed());
    let can = |g: &Option<Grant>| g.as_ref().is_some_and(Grant::allowed);
    let headline = create.as_ref().and_then(|g| g.headline("INSERT"));
    let detail = create
        .as_ref()
        .and_then(|g| g.detail("INSERT", "add records"));
    let label = collection.profile.insert_label();

    rsx! {
        div { class: "toolbar records-toolbar",
            if spec.range {
                TimeRangeControl {}
            }
            if spec.bbox {
                BboxControl {}
            }
            if spec.filter {
                FilterControl { schema: schema.clone(), open: filter_open }
            }
            if spec.sort {
                SortControl { schema: schema.clone() }
            }
            if let Some(placeholder) = spec.text {
                TextControl { placeholder }
            }
            div { class: "toolbar-spacer" }
            ColumnsControl { schema: schema.clone() }
            button {
                class: "icon-btn bordered",
                title: "Export these records",
                "aria-label": "Export",
                onclick: move |_| { nav.push(Route::DataTransfer {}); },
                Ico { icon: Icon::Upload, size: 16 }
            }
            if !details_visible {
                button {
                    class: "btn",
                    disabled: selected.is_none() || !can(&update),
                    title: if can(&update) { "Edit the selected record" } else { "UPDATE permission required" },
                    onclick: {
                        let (c, s, r) = (collection.clone(), schema.clone(), selected.clone());
                        move |_| if let Some(r) = &r { launcher.edit(&c, &s, r) }
                    },
                    Ico { icon: Icon::Pencil, size: 16 }
                    "Edit"
                }
                button {
                    class: "btn danger",
                    disabled: selected.is_none() || !can(&delete),
                    title: if can(&delete) { "Delete the selected record" } else { "DELETE permission required" },
                    onclick: {
                        let (c, r) = (collection.name.clone(), selected.clone());
                        move |_| if let Some(r) = &r { launcher.delete_record(&c, r) }
                    },
                    Ico { icon: Icon::Trash, size: 16 }
                    "Delete"
                }
            }
            button {
                class: "btn primary",
                disabled: !can(&create),
                title: headline.clone().unwrap_or_default(),
                onclick: {
                    let (c, s) = (collection.clone(), schema.clone());
                    move |_| launcher.insert(&c, &s)
                },
                Ico { icon: Icon::Plus, size: 16 }
                "{label}"
            }
        }
        if denied_create && let Some(h) = &headline {
            div { class: "write-note", role: "note",
                strong { "{h}" }
                if let Some(d) = detail {
                    span { "{d}" }
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

    async fn named(name: &str) -> Collection {
        MockConnectionService::ready()
            .catalog()
            .await
            .expect("catalog")
            .into_iter()
            .find(|c| c.name == name)
            .expect("collection")
    }

    #[tokio::test]
    async fn controls_follow_the_profile() {
        assert!(controls_for(&named("users").await).filter);
        assert!(controls_for(&named("sales").await).range);
        assert!(controls_for(&named("places").await).bbox);
        assert_eq!(
            controls_for(&named("sessions").await).text,
            Some("Filter keys…")
        );
        // A collection with search attached filters by text, not by field.
        let a = controls_for(&named("articles").await);
        assert!(!a.filter && a.text.is_some());
    }

    #[tokio::test]
    async fn only_scalar_columns_sort() {
        let svc = MockConnectionService::ready();
        let places = sortable(&svc.collection_schema("places").await.expect("schema"));
        assert!(places.contains(&"name".to_string()));
        assert!(!places.contains(&"geometry".to_string()));
        let sessions = sortable(&svc.collection_schema("sessions").await.expect("schema"));
        assert!(!sessions.contains(&"value".to_string()));
    }
}
