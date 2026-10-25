//! Administration → Backups: the snapshots a schedule takes, the restore
//! jobs, the chosen snapshot's details, and the restore preview.
//!
//! Four panels. A snapshot is a point-in-time copy kept for recovery.
//! Replication keeps live replicas and copies a mistaken write to every one
//! of them, so the page says the two are different. A restore creates a new
//! database. Restoring over an existing database is a separate review.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side, region_style};
use crate::models::admin::{AdminChange, bytes_text};
use crate::models::admin_backup::{BackupChange, BackupOverview, Snapshot};
use crate::models::format::{date_minute, duration};
use crate::routes::tabs::AdminTab;
use crate::services::backend::Backend;
use crate::state::admin::{AdminUi, pick};
use crate::state::workspace::{Surface, Workspace};
use crate::views::admin::backup_details::{RestorePanel, SnapshotDetails};
use crate::views::admin::backup_sheets::BackupSheets;
use crate::views::admin::common::{Cut, dash, unknown};
use crate::views::admin::tasks;
use crate::views::events::common::{NoticeLine, StatusText};

const SURFACE: Surface = Surface::Admin(AdminTab::Backups);
const JOBS_SIZE: PanelSize = PanelSize::height(240.0, 140.0, 460.0);
const SIDE_SIZE: PanelSize = PanelSize::width(460.0, 360.0, 640.0);
const RESTORE_SIZE: PanelSize = PanelSize::height(292.0, 220.0, 560.0);

/// The snapshot the details show: the chosen one while it is listed, else the first.
pub fn chosen<'a>(o: &'a BackupOverview, id: Option<&str>) -> Option<&'a Snapshot> {
    pick(&o.snapshots, id, |s| &s.id)
}

#[component]
pub fn BackupsTab() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut ui = use_context::<AdminUi>().backups;
    let layout = use_context::<Signal<Workspace>>();
    let epoch = use_memo(move || ui.read().epoch);

    let mut overview = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            let _conn = conn();
            let _epoch = epoch();
            async move { backend.backup_overview().await }
        }
    });
    let state = resource_state(&overview);
    let data = state.loaded().cloned();
    let u = ui.read().clone();
    let selected = data
        .as_ref()
        .and_then(|o| chosen(o, u.selected.as_deref()).cloned());
    let side_visible = {
        let l = layout.read();
        l.is_visible(SURFACE, "details") || l.is_visible(SURFACE, "restore")
    };
    let side_style = region_style(&layout.read(), SURFACE, "side", SIDE_SIZE);
    let create = {
        let backend = backend.clone();
        let database = selected
            .as_ref()
            .map(|s| s.database.clone())
            .unwrap_or_else(|| "analytics".to_string());
        move |_| {
            let (backend, change) = (
                backend.clone(),
                AdminChange::Backup(BackupChange::CreateSnapshot {
                    database: database.clone(),
                }),
            );
            spawn_forever(async move { tasks::open_review(ui, backend, change).await });
        }
    };

    rsx! {
        div { class: "page-body",
            div { class: "workbench events-wb",
                div { class: "workbench col",
                    Panel {
                        surface: SURFACE,
                        id: "snapshots",
                        title: "Scheduled snapshots",
                        class: "fill",
                        actions: rsx! {
                            button { class: "btn primary", onclick: create.clone(),
                                Ico { icon: Icon::Plus, size: 16 }
                                "Create snapshot"
                            }
                        },
                        NoticeLine {
                            notice: u.notice.clone(),
                            on_dismiss: move |_| ui.write().notice = None,
                        }
                        AsyncView {
                            loading: state.is_loading(),
                            empty: state.is_empty(),
                            error: state.error_message(),
                            retriable: state.is_retriable(),
                            on_retry: move |_| overview.restart(),
                            empty_message: "No snapshot exists. Create one, or manage the schedule.",
                        }
                        if let Some(o) = data.as_ref() {
                            div { class: "dt-wrap",
                                table { class: "dt cl-table ad-table ad-snapshots",
                                    thead {
                                        tr {
                                            th { "Name" }
                                            th { "Schedule" }
                                            th { "Status" }
                                            th { "Size" }
                                            th { "Encrypted" }
                                            th { "Retention" }
                                            th { "Last restore test" }
                                        }
                                    }
                                    tbody {
                                        for s in o.snapshots.iter() {
                                            {
                                                let id = s.id.clone();
                                                let is_selected = selected.as_ref().is_some_and(|x| x.id == s.id);
                                                rsx! {
                                                    tr {
                                                        key: "{s.id}",
                                                        class: if is_selected { "clickable selected" } else { "clickable" },
                                                        "aria-selected": "{is_selected}",
                                                        tabindex: "0",
                                                        onclick: move |_| ui.write().selected = Some(id.clone()),
                                                        td { class: "key", Cut { text: s.name.clone() } }
                                                        td { Cut { text: s.schedule.clone() } }
                                                        td { StatusText { tone: s.state.tone(), text: s.state.label().to_string() } }
                                                        td {
                                                            match s.size_bytes {
                                                                Some(b) => rsx! { "{bytes_text(b)}" },
                                                                None => unknown(),
                                                            }
                                                        }
                                                        td { if s.cipher.is_some() { "Yes" } else { "No" } }
                                                        td { "{s.retention_days} days" }
                                                        td {
                                                            match s.last_restore_test {
                                                                Some(t) => rsx! { "{date_minute(t, 0)} UTC" },
                                                                None => rsx! { span { class: "muted", "Not reported" } },
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            div { class: "callout info ad-backup-note",
                                Ico { icon: Icon::Info, size: 16 }
                                span {
                                    "A snapshot is a point-in-time copy for recovery. Replication keeps live replicas in step and copies a mistaken write to every replica, so it is no backup. See Cluster for replication."
                                }
                            }
                        }
                    }
                    ResizeHandle { surface: SURFACE, target: "jobs", size: JOBS_SIZE, side: Side::After }
                    Panel {
                        surface: SURFACE,
                        id: "jobs",
                        title: "Restore jobs",
                        size: JOBS_SIZE,
                        if let Some(o) = data.as_ref() {
                            if o.restore_jobs.is_empty() {
                                div { class: "async-empty", "No restore has run." }
                            } else {
                                div { class: "dt-wrap",
                                    table { class: "dt cl-table ad-table ad-jobs",
                                        thead {
                                            tr {
                                                th { "Job" }
                                                th { "Target" }
                                                th { "State" }
                                                th { "Created (UTC)" }
                                                th { "Duration" }
                                                th { "Report" }
                                            }
                                        }
                                        tbody {
                                            for j in o.restore_jobs.iter() {
                                                tr { key: "{j.id}",
                                                    td { class: "key", "{j.id}" }
                                                    td { Cut { text: j.target.clone() } }
                                                    td { StatusText { tone: j.state.tone(), text: j.state.label().to_string() } }
                                                    td { "{date_minute(j.created_at, 0)} UTC" }
                                                    td {
                                                        match j.duration_secs {
                                                            Some(s) => rsx! { "{duration(s)}" },
                                                            None => dash(),
                                                        }
                                                    }
                                                    td {
                                                        button {
                                                            class: "link-btn",
                                                            disabled: true,
                                                            title: "The restore report is not designed yet",
                                                            Ico { icon: Icon::FileText, size: 16 }
                                                            "View report"
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
                }
                if side_visible {
                    ResizeHandle { surface: SURFACE, target: "side", size: SIDE_SIZE, side: Side::After }
                    div { class: "workbench col ad-side", style: "{side_style}",
                        Panel {
                            surface: SURFACE,
                            id: "details",
                            title: "Snapshot details",
                            class: "fill",
                            if let (Some(s), Some(o)) = (selected.clone(), data.clone()) {
                                SnapshotDetails { snapshot: s, overview: o }
                            }
                        }
                        ResizeHandle { surface: SURFACE, target: "restore", size: RESTORE_SIZE, side: Side::After }
                        Panel {
                            surface: SURFACE,
                            id: "restore",
                            title: "Restore preview",
                            size: RESTORE_SIZE,
                            if let (Some(s), Some(o)) = (selected.clone(), data.clone()) {
                                RestorePanel { snapshot: s, overview: o }
                            }
                        }
                    }
                }
                if let Some(o) = data.clone() {
                    BackupSheets { overview: o }
                }
            }
        }
    }
}
