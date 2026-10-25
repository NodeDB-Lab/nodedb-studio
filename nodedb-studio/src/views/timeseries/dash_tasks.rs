//! Dashboard writes. Each runs under `spawn_forever` and touches root state
//! only, so a save finishes when the user leaves the page mid-save.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::models::timeseries::TimeSpan;
use crate::models::ts_dashboard::Dashboard;
use crate::services::backend::Backend;
use crate::services::error::StudioError;
use crate::state::timeseries::{After, DashDialog, DashUi, Notice, SaveState};
use crate::views::timeseries::dash_ops::proceed;

/// Store the open document over itself. A document with no identity asks
/// for a name first. `then` runs once it is stored.
pub async fn save(
    mut ui: Signal<DashUi>,
    backend: Rc<dyn Backend>,
    then: Option<After>,
    range: TimeSpan,
) {
    let Some(doc) = ui.peek().open.clone() else {
        return;
    };
    if doc.id.is_none() {
        ui.write().dialog = Some(DashDialog::SaveAs {
            name: if doc.name == "Untitled dashboard" {
                String::new()
            } else {
                doc.name.clone()
            },
            error: None,
            then,
        });
        return;
    }
    ui.write().save = SaveState::Saving;
    let result = backend.save_dashboard(&doc).await;
    let mut w = ui.write();
    match result {
        Ok(saved) => {
            w.notice = Some(Notice::ok(format!(
                "Saved {} as revision {}.",
                saved.name, saved.revision
            )));
            w.baseline = Some(saved.clone());
            w.open = Some(saved);
            w.save = SaveState::Idle;
            w.dialog = None;
            w.library_epoch += 1;
            if let Some(after) = then {
                proceed(&mut w, &after, range);
            }
        }
        Err(e @ StudioError::StaleRevision { .. }) => {
            // Another session saved first. Nothing was overwritten.
            w.save = SaveState::Failed(e.to_string());
            w.dialog = Some(DashDialog::Conflict);
        }
        Err(e) => {
            // The draft stays: Retry or Save as are both still open.
            w.save = SaveState::Failed(e.to_string());
        }
    }
}

/// Store the open document as a new document under `name`.
pub async fn save_as(
    mut ui: Signal<DashUi>,
    backend: Rc<dyn Backend>,
    name: String,
    then: Option<After>,
    range: TimeSpan,
) {
    let Some(doc) = ui.peek().open.clone() else {
        return;
    };
    ui.write().save = SaveState::Saving;
    let result = backend.save_dashboard_as(&name, &doc).await;
    let mut w = ui.write();
    match result {
        Ok(saved) => {
            w.notice = Some(Notice::ok(format!(
                "Saved {} as a new dashboard.",
                saved.name
            )));
            w.open_id.clone_from(&saved.id);
            w.selected.clone_from(&saved.id);
            w.baseline = Some(saved.clone());
            w.open = Some(saved);
            w.save = SaveState::Idle;
            w.dialog = None;
            w.library_epoch += 1;
            if let Some(after) = then {
                proceed(&mut w, &after, range);
            }
        }
        Err(e) => {
            w.save = SaveState::Idle;
            w.dialog = Some(DashDialog::SaveAs {
                name,
                error: Some(e.to_string()),
                then,
            });
        }
    }
}

/// Rename a stored document. The working copy takes the name too, so the
/// toolbar and the library agree.
pub async fn rename(
    mut ui: Signal<DashUi>,
    backend: Rc<dyn Backend>,
    id: String,
    revision: u64,
    name: String,
) {
    let result = backend.rename_dashboard(&id, revision, &name).await;
    let mut w = ui.write();
    match result {
        Ok(renamed) => {
            if w.open_id.as_deref() == Some(id.as_str()) {
                if let Some(open) = w.open.as_mut() {
                    open.name.clone_from(&renamed.name);
                    open.revision = renamed.revision;
                    open.saved_at = renamed.saved_at;
                }
                if let Some(base) = w.baseline.as_mut() {
                    base.name.clone_from(&renamed.name);
                    base.revision = renamed.revision;
                    base.saved_at = renamed.saved_at;
                }
            }
            w.notice = Some(Notice::ok(format!("Renamed to {}.", renamed.name)));
            w.dialog = None;
            w.library_epoch += 1;
        }
        Err(e) => {
            w.dialog = Some(DashDialog::Rename {
                name,
                error: Some(e.to_string()),
            });
        }
    }
}

/// Store a copy of a stored document and open nothing: the library lists it.
pub async fn duplicate(mut ui: Signal<DashUi>, backend: Rc<dyn Backend>, id: String) {
    let result = backend.duplicate_dashboard(&id).await;
    let mut w = ui.write();
    match result {
        Ok(copy) => {
            w.selected.clone_from(&copy.id);
            w.notice = Some(Notice::ok(format!("Created {}.", copy.name)));
            w.library_epoch += 1;
        }
        Err(e) => w.notice = Some(Notice::err(e.to_string())),
    }
}

/// Delete a stored document. If it is open, the page falls back to the
/// empty state: the working copy has no document left to save into.
pub async fn delete(mut ui: Signal<DashUi>, backend: Rc<dyn Backend>, id: String, name: String) {
    let result = backend.delete_dashboard(&id).await;
    let mut w = ui.write();
    w.dialog = None;
    match result {
        Ok(()) => {
            if w.open_id.as_deref() == Some(id.as_str()) {
                w.open_id = None;
                w.open = None;
                w.baseline = None;
                w.editor = None;
            }
            if w.selected.as_deref() == Some(id.as_str()) {
                w.selected = None;
            }
            w.notice = Some(Notice::ok(format!("Deleted {name}. Its panels are gone.")));
            w.library_epoch += 1;
        }
        Err(e) => w.notice = Some(Notice::err(e.to_string())),
    }
}

/// The stored document the working copy came from, re-read: the way out of
/// a save conflict is to take what the other session saved.
pub async fn reload(mut ui: Signal<DashUi>, backend: Rc<dyn Backend>, id: String) {
    match backend.dashboard(&id).await {
        Ok(doc) => adopt_reloaded(&mut ui.write(), doc),
        Err(e) => ui.write().notice = Some(Notice::err(e.to_string())),
    }
}

fn adopt_reloaded(ui: &mut DashUi, doc: Dashboard) {
    ui.baseline = Some(doc.clone());
    ui.open = Some(doc);
    ui.dialog = None;
    ui.save = SaveState::Idle;
    ui.removed = None;
    ui.editor = None;
    ui.notice = Some(Notice::ok(
        "Reloaded the stored dashboard. Your unsaved changes are gone.",
    ));
}
