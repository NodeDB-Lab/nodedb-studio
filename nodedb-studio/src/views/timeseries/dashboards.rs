//! Dashboards: named documents of query panels. The library lists the saved
//! ones. Opening one restores its queries and layout and runs them fresh.
//! Moving, resizing, and editing panels changes the document, which stays
//! unsaved until Save dashboard. Hiding a panel with × is personal, and
//! never changes the document.

use std::rc::Rc;
use std::time::Duration;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::models::format::date_minute;
use crate::models::records::Filter;
use crate::models::timeseries::{TimeCatalog, TimeSpan};
use crate::models::ts_dashboard::{Dashboard, DashboardSummary, Refresh};
use crate::routes::tabs::TimeseriesTab;
use crate::services::backend::Backend;
use crate::state::connection::ActiveConnection;
use crate::state::panel_registry::{DynamicPanel, DynamicPanels};
use crate::state::timeseries::{DashUi, SaveState, TimeseriesUi};
use crate::state::workspace::Surface;
use crate::views::timeseries::dash_dialogs::DashDialogs;
use crate::views::timeseries::dash_ops::{
    adopt, new_panel, panel_key, request_new, request_open, take_pending_add, undo_remove,
};
use crate::views::timeseries::dash_panel::{DashPanel, PanelDivider};
use crate::views::timeseries::dash_tasks::{duplicate, save};
use crate::views::timeseries::library::LibraryBody;
use crate::views::timeseries::notice::TsNotice;
use crate::views::timeseries::panel_editor::PanelEditorSheet;
use crate::views::timeseries::toolbar::{FilterAdder, RangeButton};

const SURFACE: Surface = Surface::Timeseries(TimeseriesTab::Dashboards);
const LIBRARY_SIZE: PanelSize = PanelSize::width(300.0, 240.0, 420.0);

/// What the title row says about the document's persistence. Loading data
/// is separate: a panel can still be loading in any of these.
pub fn state_text(ui: &DashUi) -> (&'static str, String) {
    let Some(doc) = &ui.open else {
        return ("idle", String::new());
    };
    match &ui.save {
        SaveState::Saving => ("info", "Saving…".to_string()),
        SaveState::Failed(_) => ("err", "Save failed. Retry or Save as.".to_string()),
        SaveState::Idle => {
            let saved = doc.saved_at.map(|t| {
                format!(
                    "saved {} UTC",
                    date_minute(t, 0).get(5..).unwrap_or_default()
                )
            });
            match (doc.id.is_some(), ui.modified()) {
                (false, _) => ("warn", "Not saved yet".to_string()),
                (true, true) => ("warn", format!("Modified · {}", saved.unwrap_or_default())),
                (true, false) => ("ok", format!("Saved · {}", saved.unwrap_or_default())),
            }
        }
    }
}

#[component]
pub fn DashboardsTab() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let ts = use_context::<TimeseriesUi>();
    let mut dash = ts.dash;
    let mut extra = use_context::<DynamicPanels>().0;

    let database = active
        .read()
        .as_ref()
        .map(|c| c.current_database.clone())
        .unwrap_or_default();

    // The library and the catalog belong to the database in scope.
    let mut library_read = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let _scope = (
                conn(),
                active.read().as_ref().map(|c| c.current_database.clone()),
            );
            let _epoch = dash.read().library_epoch;
            async move { backend.dashboards().await }
        })
    };
    let catalog_read = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let _scope = (
                conn(),
                active.read().as_ref().map(|c| c.current_database.clone()),
            );
            async move { backend.time_sources().await }
        })
    };
    let lib_state = resource_state(&library_read);
    let summaries: Vec<DashboardSummary> = lib_state.loaded().cloned().unwrap_or_default();
    let catalog: Option<TimeCatalog> = catalog_read
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned();
    let now = catalog.as_ref().map_or(0, |c| c.observed_at);
    let default_range = match &catalog {
        Some(c) => crate::models::timeseries::Preset::Last7Days.span(c.observed_at),
        None => TimeSpan::new(0, 7 * 86_400),
    };

    // Load the document that was asked for, and adopt it.
    use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            let (want, loaded) = {
                let d = dash.read();
                (d.open_id.clone(), d.open.is_some())
            };
            async move {
                let (Some(id), false) = (want, loaded) else {
                    return;
                };
                match backend.dashboard(&id).await {
                    Ok(doc) => {
                        let mut w = dash.write();
                        // Only if it is still the one wanted: a newer open
                        // replaces a slow earlier one.
                        if w.open_id.as_deref() == Some(id.as_str()) && w.open.is_none() {
                            adopt(&mut w, doc);
                        }
                    }
                    Err(e) => {
                        let mut w = dash.write();
                        w.open_id = None;
                        w.notice = Some(crate::state::timeseries::Notice::err(e.to_string()));
                    }
                }
            }
        }
    });

    // Requests that arrive while this page is away: another page's panel,
    // a New, an Open, or a Duplicate.
    use_effect({
        let backend = backend.clone();
        move || {
            let (new, open, add, dup) = {
                let d = dash.read();
                (
                    d.pending_new,
                    d.pending_open,
                    d.pending_add.is_some() && d.open.is_some(),
                    d.pending_duplicate.clone(),
                )
            };
            if !(new || open || add || dup.is_some()) {
                return;
            }
            let mut w = dash.write();
            if new {
                w.pending_new = false;
                request_new(&mut w, default_range);
            } else if open {
                w.pending_open = false;
                if let Some(id) = w.selected.clone() {
                    request_open(&mut w, &id);
                }
            }
            if w.pending_add.is_some() && !w.pending_new && !w.pending_open {
                take_pending_add(&mut w);
            }
            let duplicate_id = w.pending_duplicate.take();
            drop(w);
            if let Some(id) = duplicate_id {
                spawn_forever(duplicate(dash, backend.clone(), id));
            }
        }
    });

    // The View menu lists the open document's panels by name.
    use_effect(move || {
        let d = dash.read();
        let list: Vec<DynamicPanel> = d
            .open
            .as_ref()
            .map(|doc| {
                doc.panels
                    .iter()
                    .map(|p| DynamicPanel {
                        surface: SURFACE,
                        id: panel_key(doc, &p.id),
                        label: p.title.clone(),
                        hint: format!("Show this panel of {}", doc.name),
                    })
                    .collect()
            })
            .unwrap_or_default();
        drop(d);
        if *extra.peek() != list {
            extra.set(list);
        }
    });
    use_drop(move || {
        if !extra.peek().is_empty() {
            extra.set(Vec::new());
        }
    });

    // The document's refresh interval re-runs its panels. Each panel reads
    // the counter, and a panel already waiting is never run again.
    let mut tick = use_signal(|| 0u32);
    use_resource(move || {
        let every = dash.read().open.as_ref().and_then(|d| d.refresh.seconds());
        async move {
            let Some(secs) = every else { return };
            loop {
                tokio::time::sleep(Duration::from_secs(secs)).await;
                let next = *tick.peek() + 1;
                tick.set(next);
            }
        }
    });

    let d = dash.read().clone();
    let open_doc = d.open.clone();

    rsx! {
        div { class: "page-body ts-page",
            TsNotice {
                notice: d.notice.clone(),
                on_dismiss: move |_| dash.write().notice = None,
                undo: d.removed.is_some(),
                on_undo: move |_| undo_remove(&mut dash.write()),
            }
            div { class: "workbench events-wb ts-dash",
                Panel {
                    surface: SURFACE,
                    id: "library",
                    title: "Saved dashboards",
                    size: LIBRARY_SIZE,
                    class: "ts-library-panel",
                    actions: rsx! {
                        button {
                            class: "btn",
                            title: "Start an empty dashboard draft. Nothing is created until you save it.",
                            onclick: move |_| request_new(&mut dash.write(), default_range),
                            Ico { icon: Icon::Plus, size: 16 }
                            "New"
                        }
                    },
                    AsyncView {
                        loading: lib_state.is_loading(),
                        empty: false,
                        error: lib_state.error_message(),
                        retriable: lib_state.is_retriable(),
                        on_retry: move |_| library_read.restart(),
                    }
                    if !lib_state.is_loading() && lib_state.error_message().is_none() {
                        LibraryBody {
                            rows: summaries.clone(),
                            ui: d.clone(),
                        }
                    }
                }
                ResizeHandle { surface: SURFACE, target: "library", size: LIBRARY_SIZE, side: Side::Before }
                div { class: "ts-dash-main",
                    match open_doc.clone() {
                        Some(doc) => rsx! {
                            DocBar { doc: doc.clone(), ui: d.clone(), range: default_range, catalog: catalog.clone() }
                            FilterBar { doc: doc.clone(), now, catalog: catalog.clone() }
                            div { class: "ts-dash-panels",
                                if doc.panels.is_empty() {
                                    div { class: "empty-state ts-dash-empty",
                                        Ico { icon: Icon::ChartColumn, size: 24 }
                                        h2 { "No panels yet" }
                                        p { "Add a panel to choose a collection, shape its query, preview it, and add it. Nothing is stored until you save the dashboard." }
                                    }
                                }
                                for (i , p) in doc.panels.iter().enumerate() {
                                    DashPanel {
                                        key: "{panel_key(&doc, &p.id)}",
                                        doc: doc.clone(),
                                        panel: p.clone(),
                                        first: i == 0,
                                        last: i + 1 == doc.panels.len(),
                                        refresh: tick(),
                                        menu_open: d.menu.as_deref() == Some(p.id.as_str()),
                                    }
                                    if i + 1 < doc.panels.len() {
                                        PanelDivider { key: "div-{p.id}", panel_id: p.id.clone(), height: p.height }
                                    }
                                }
                            }
                            div { class: "ts-dash-foot", "Panel positions and sizes are saved with this dashboard. Hiding a panel with × is personal and is not saved." }
                        },
                        None => rsx! {
                            div { class: "empty-state",
                                Ico { icon: Icon::ChartColumn, size: 24 }
                                h2 { "No dashboard open" }
                                p { "Select a dashboard and choose Open dashboard. It runs fresh queries when it opens. Or start a new one." }
                                div { class: "actions",
                                    button {
                                        class: "btn",
                                        onclick: move |_| request_new(&mut dash.write(), default_range),
                                        Ico { icon: Icon::Plus, size: 16 }
                                        "New dashboard"
                                    }
                                }
                            }
                        },
                    }
                }
                if let (Some(editor), Some(doc), Some(c)) = (d.editor.clone(), open_doc.clone(), catalog.clone()) {
                    PanelEditorSheet {
                        catalog: c,
                        doc,
                        editor,
                        database: database.clone(),
                        on_close: move |_| dash.write().editor = None,
                    }
                }
                if let Some(dialog) = d.dialog.clone() {
                    DashDialogs { dialog, ui: d.clone(), summaries: summaries.clone(), range: default_range }
                }
            }
        }
    }
}

/// The document's title, its state, and the actions that persist it.
#[component]
fn DocBar(doc: Dashboard, ui: DashUi, range: TimeSpan, catalog: Option<TimeCatalog>) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let ts = use_context::<TimeseriesUi>();
    let mut dash = ts.dash;
    let (tone, text) = state_text(&ui);
    let saving = ui.save == SaveState::Saving;
    let failed = matches!(ui.save, SaveState::Failed(_));
    let can_save = doc.id.is_none() || ui.modified() || failed;

    rsx! {
        div { class: "ts-docbar",
            h2 { class: "ts-doc-title", "{doc.name}" }
            span { class: "status ts-doc-state", role: "status",
                span { class: "status-dot {tone}" }
                "{text}"
            }
            div { class: "toolbar-spacer" }
            if failed {
                button {
                    class: "btn",
                    title: "Try the save again",
                    onclick: {
                        let backend = backend.clone();
                        move |_| { spawn_forever(save(dash, backend.clone(), None, range)); }
                    },
                    Ico { icon: Icon::RefreshCw, size: 16 }
                    "Retry"
                }
            }
            if ui.modified() {
                button {
                    class: "icon-btn",
                    title: "Revert: discard the unsaved changes",
                    "aria-label": "Revert unsaved changes",
                    onclick: move |_| crate::views::timeseries::dash_ops::revert(&mut dash.write()),
                    Ico { icon: Icon::RotateCcw, size: 16 }
                }
            }
            button {
                class: "btn",
                title: "Save a copy under another name",
                onclick: move |_| {
                    dash.write().dialog = Some(crate::state::timeseries::DashDialog::SaveAs {
                        name: String::new(),
                        error: None,
                        then: None,
                    });
                },
                "Save as…"
            }
            button {
                class: "btn",
                title: "Add a panel: pick a collection, shape the query, preview, add",
                onclick: {
                    let catalog = catalog.clone();
                    move |_| {
                        if let Some(q) = catalog
                            .as_ref()
                            .and_then(|c| c.sources.iter().find(|s| s.eligible()).and_then(|s| s.default_query(c.observed_at)))
                        {
                            new_panel(&mut dash.write(), q);
                        }
                    }
                },
                disabled: catalog.is_none(),
                Ico { icon: Icon::Plus, size: 16 }
                "Panel"
            }
            button {
                class: "btn primary",
                disabled: !can_save || saving,
                title: if can_save { "Save the queries, layout, and settings" } else { "Nothing has changed since the last save" },
                onclick: {
                    let backend = backend.clone();
                    move |_| { spawn_forever(save(dash, backend.clone(), None, range)); }
                },
                Ico { icon: Icon::Save, size: 16 }
                "Save dashboard"
            }
        }
    }
}

/// The range, refresh, and filters every panel shares.
#[component]
fn FilterBar(doc: Dashboard, now: i64, catalog: Option<TimeCatalog>) -> Element {
    let ts = use_context::<TimeseriesUi>();
    let mut dash = ts.dash;
    // A filter reaches every panel, so it must name a field every panel's
    // collection has.
    let choices: Vec<String> = match &catalog {
        Some(c) => {
            let mut common: Option<Vec<String>> = None;
            for name in doc.collections() {
                let fields: Vec<String> = c
                    .source(&name)
                    .map(|s| {
                        s.fields
                            .iter()
                            .filter(|f| f.type_label.starts_with("string"))
                            .map(|f| f.name.clone())
                            .collect()
                    })
                    .unwrap_or_default();
                common = Some(match common {
                    None => fields,
                    Some(prev) => prev.into_iter().filter(|f| fields.contains(f)).collect(),
                });
            }
            common.unwrap_or_default()
        }
        None => Vec::new(),
    };

    rsx! {
        div { class: "ts-toolbar ts-filterbar",
            RangeButton {
                span: doc.range,
                now,
                invalid: false,
                on_apply: move |span: TimeSpan| {
                    if let Some(d) = dash.write().open.as_mut() {
                        d.range = span;
                    }
                },
            }
            span { class: "ts-utc", title: "Buckets align to UTC. Bounds are half-open: start included, end excluded.", "UTC" }
            div { class: "ts-ctl",
                label { r#for: "ts-dash-refresh", "Refresh" }
                select {
                    id: "ts-dash-refresh",
                    class: "select native",
                    onchange: move |e| {
                        let v = e.value();
                        let next = Refresh::ALL.into_iter().find(|r| r.label() == v).unwrap_or(Refresh::Off);
                        if let Some(d) = dash.write().open.as_mut() {
                            d.refresh = next;
                        }
                    },
                    for r in Refresh::ALL {
                        option { key: "{r.label()}", value: "{r.label()}", selected: r == doc.refresh, "{r.label()}" }
                    }
                }
            }
            for f in doc.filters.iter() {
                {
                    let field = f.field.clone();
                    let label = f.label();
                    rsx! {
                        span { key: "{field}", class: "chip", title: "Applies to every panel",
                            Ico { icon: Icon::Filter, size: 16 }
                            "{label}"
                            button {
                                class: "chip-remove",
                                "aria-label": "Remove filter {label}",
                                onclick: move |_| {
                                    if let Some(d) = dash.write().open.as_mut() {
                                        d.filters.retain(|x| x.field != field);
                                    }
                                },
                                Ico { icon: Icon::X, size: 16 }
                            }
                        }
                    }
                }
            }
            FilterAdder {
                choices,
                on_add: move |f: Filter| {
                    if let Some(d) = dash.write().open.as_mut() {
                        d.filters.retain(|x| x.field != f.field);
                        d.filters.push(f);
                    }
                },
            }
        }
    }
}
