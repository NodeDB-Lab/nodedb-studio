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
use crate::models::timeseries::{TimeCatalog, TimeSpan};
use crate::models::ts_dashboard::DashboardSummary;
use crate::routes::tabs::TimeseriesTab;
use crate::services::backend::Backend;
use crate::state::connection::ActiveConnection;
use crate::state::panel_registry::{DynamicPanel, DynamicPanels};
use crate::state::timeseries::TimeseriesUi;
use crate::state::workspace::Surface;
use crate::views::timeseries::dash_bars::{DocBar, FilterBar};
use crate::views::timeseries::dash_dialogs::DashDialogs;
use crate::views::timeseries::dash_ops::{
    adopt, panel_key, request_new, request_open, take_pending_add, undo_remove,
};
use crate::views::timeseries::dash_panel::{DashPanel, PanelDivider};
use crate::views::timeseries::dash_tasks::duplicate;
use crate::views::timeseries::library::LibraryBody;
use crate::views::timeseries::notice::TsNotice;
use crate::views::timeseries::panel_editor::PanelEditorSheet;

const SURFACE: Surface = Surface::Timeseries(TimeseriesTab::Dashboards);
const LIBRARY_SIZE: PanelSize = PanelSize::width(300.0, 240.0, 420.0);

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
