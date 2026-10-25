//! The Snapshot details inspector and the Restore preview form.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::clipboard::use_copier;
use crate::components::icon::{Ico, Icon};
use crate::models::admin::{AdminChange, bytes_text};
use crate::models::admin_backup::{
    BackupChange, BackupOverview, RestoreDraft, RestoreRequest, ScheduleDraft, Snapshot,
};
use crate::models::format::{date_minute, date_second};
use crate::routes::Route;
use crate::routes::tabs::AdminTab;
use crate::services::backend::Backend;
use crate::state::admin::AdminUi;
use crate::state::workspace::{Surface, Workspace};
use crate::views::admin::common::{Cut, unknown};
use crate::views::admin::tasks;
use crate::views::events::common::{FormField, StatusBadge, StatusText};

#[component]
pub fn SnapshotDetails(snapshot: Snapshot, overview: BackupOverview) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let nav = use_navigator();
    let mut ui = use_context::<AdminUi>().backups;
    let copier = use_copier();
    let mut creds = use_context::<AdminUi>().credentials;
    let mut ws = use_context::<Signal<Workspace>>();
    let s = &snapshot;
    let create = {
        let database = s.database.clone();
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
    let schedule = overview.schedule.clone();
    let scheduled = s.is_scheduled();
    rsx! {
        div { class: "inspector ad-details",
            div { class: "inspector-head",
                h3 { class: "inspector-title", title: "{s.name}", "{s.name}" }
                StatusBadge { tone: s.state.tone(), text: s.state.label().to_string() }
            }
            div { class: "kv",
                div { class: "kv-row", span { "Source database" } span { "{s.database}" } }
                div { class: "kv-row", span { "Snapshot type" } span { "Consistent point-in-time" } }
                div { class: "kv-row",
                    span { "Snapshot ID" }
                    Cut { class: "mono", text: s.id.clone() }
                }
                div { class: "kv-row",
                    span { "Checksums" }
                    StatusText { tone: s.checksums.tone(), text: s.checksums.label().to_string() }
                }
                div { class: "kv-row",
                    span { "Storage destination" }
                    span { class: "ad-copyline",
                        Cut { class: "mono", text: s.destination.clone() }
                        button {
                            class: "icon-btn sm",
                            title: "Copy the destination",
                            "aria-label": "Copy the storage destination",
                            onclick: {
                                let text = s.destination.clone();
                                move |_| copier.copy(text.clone())
                            },
                            Ico { icon: Icon::Copy, size: 16 }
                        }
                    }
                }
                div { class: "kv-row",
                    span { "Credential alias" }
                    button {
                        class: "link-btn",
                        title: "Open this credential",
                        onclick: {
                            let alias = s.credential.clone();
                            move |_| {
                                creds.write().selected = Some(alias.clone());
                                nav.push(Route::Admin { tab: AdminTab::Credentials });
                            }
                        },
                        "{s.credential}"
                        Ico { icon: Icon::ExternalLink, size: 16 }
                    }
                }
                div { class: "kv-row",
                    span { "Encrypted" }
                    match s.cipher.as_ref() {
                        Some(c) => rsx! { span { "Yes ({c})" } },
                        None => rsx! { span { "No" } },
                    }
                }
                div { class: "kv-row", span { "Retention" } span { "{s.retention_days} days" } }
                div { class: "kv-row", span { "Created (UTC)" } span { "{date_second(s.created_at, 0)}" } }
                div { class: "kv-row",
                    span { "Size" }
                    match s.size_bytes {
                        Some(b) => rsx! { span { "{bytes_text(b)}" } },
                        None => unknown(),
                    }
                }
                div { class: "kv-row",
                    span { "Last restore test" }
                    match s.last_restore_test {
                        Some(t) => rsx! { span { "{date_minute(t, 0)} UTC" } },
                        None => rsx! { span { class: "muted", "Not reported" } },
                    }
                }
            }
            div { class: "ad-buttons",
                button { class: "btn sm primary", onclick: create,
                    "Create snapshot"
                }
                button {
                    class: "btn sm",
                    disabled: !scheduled,
                    title: if scheduled { "Change when this schedule takes a snapshot" } else { "An on-demand snapshot has no schedule" },
                    onclick: move |_| ui.write().schedule = Some(ScheduleDraft::of(&schedule)),
                    "Manage schedule"
                }
                button {
                    class: "btn sm",
                    title: "Preview a restore of this snapshot into a new database",
                    onclick: move |_| {
                        ws.write().set_visible(Surface::Admin(AdminTab::Backups), "restore", true);
                    },
                    "Restore preview"
                }
            }
        }
    }
}

#[component]
pub fn RestorePanel(snapshot: Snapshot, overview: BackupOverview) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let mut ui = use_context::<AdminUi>().backups;
    let draft = RestoreDraft {
        target: ui.read().restore_target.clone(),
    };
    let issue = draft.target_issue(&overview.databases);
    let target = draft.target.trim().to_string();
    let preview = {
        let request = RestoreRequest {
            snapshot: snapshot.id.clone(),
            target: target.clone(),
            point: overview.restore_to,
        };
        move |_| {
            let (backend, change) = (
                backend.clone(),
                AdminChange::Backup(BackupChange::Restore(request.clone())),
            );
            spawn_forever(async move { tasks::open_review(ui, backend, change).await });
        }
    };
    rsx! {
        div { class: "ad-restore",
            div { class: "ad-form",
                FormField {
                    label: "New database name",
                    id: "rs-target",
                    error: issue.clone(),
                    class: "ad-inline",
                    input {
                        id: "rs-target",
                        class: "input mono",
                        value: "{draft.target}",
                        spellcheck: false,
                        oninput: move |e| ui.write().restore_target = e.value(),
                    }
                }
                div { class: "kv",
                    div { class: "kv-row",
                        span { "Target" }
                        span { "New database ({target})" }
                    }
                }
            }
            div { class: "callout warn ad-restore-note",
                Ico { icon: Icon::TriangleAlert, size: 16 }
                span { "Restoring to an existing database requires a separate review." }
            }
            div { class: "kv",
                div { class: "kv-row",
                    span { title: "Available restore points", "Restore points" }
                    span { class: "ad-range",
                        "{date_minute(overview.restore_from, 0)} – {date_minute(overview.restore_to, 0)} UTC"
                    }
                }
            }
            div { class: "ad-buttons",
                button {
                    class: "btn primary",
                    disabled: issue.is_some(),
                    title: "Read what the restore would create. Nothing starts yet.",
                    onclick: preview,
                    Ico { icon: Icon::Play, size: 16 }
                    "Preview restore"
                }
            }
        }
    }
}
