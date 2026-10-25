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
use crate::services::backend::Backend;
use crate::state::explorer::{
    RecordView, RecordsEpoch, Selected, TransferNotice, TransferTab, select_collection,
};
use crate::state::preferences::Preferences;
use crate::state::workspace::Surface;
use crate::views::explorer::transfer_form::ExportForm;
use crate::views::explorer::transfer_jobs::{HistoryTable, JobsTable, recent};
use crate::views::explorer::transfer_panels::{ExportPane, ImportPane, PageTabs};

const SURFACE: Surface = Surface::DataTransfer;
const JOBS_SIZE: PanelSize = PanelSize::width(560.0, 380.0, 760.0);

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
        move |()| {
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

    let all_jobs = jobs_state.loaded().cloned().unwrap_or_default();
    let background: Vec<_> = recent(&all_jobs).into_iter().cloned().collect();
    let current_schema = schema_state.loaded().cloned();
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
                                ExportPane {
                                    form,
                                    schema: current_schema.clone(),
                                    filter: filter.clone(),
                                    request: request.read().clone(),
                                    preview: preview_state.clone(),
                                    on_start: start,
                                }
                            },
                            TransferTab::Import => rsx! {
                                ImportPane { name: c.name.clone() }
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
