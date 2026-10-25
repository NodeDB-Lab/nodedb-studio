//! The change feed page's writes. Each runs under `spawn_forever` and touches
//! root state only, so it finishes when the user leaves the page mid-run.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::services::backend::Backend;
use crate::state::events::feeds::{EditorMode, FeedsUi, Notice};

/// Select feed `id`. The tail belongs to one feed, so selecting another
/// ends it.
pub async fn select(mut ui: Signal<FeedsUi>, backend: Rc<dyn Backend>, id: String) {
    let session = {
        let mut w = ui.write();
        w.selected = Some(id.clone());
        w.selected_event = None;
        w.attempts_for_event = false;
        w.confirm_delete = false;
        w.notice = None;
        if w.tail.is_for(&id) {
            None
        } else {
            w.tail.session.clone()
        }
    };
    if session.is_some() {
        stop_tail(ui, backend).await;
    }
}

/// Open a Studio observation session on `feed` and start the tail.
pub async fn start_tail(mut ui: Signal<FeedsUi>, backend: Rc<dyn Backend>, feed: String) {
    // The previous session, if any, ends first: one tail at a time.
    stop_tail(ui, backend.clone()).await;
    match backend.open_feed_session(&feed).await {
        Ok(session) => ui.write().tail.start(&feed, session),
        Err(e) => {
            ui.write().notice = Some(Notice::err(format!("Could not start the tail: {e}")));
        }
    }
}

/// Close Studio's session. The feed keeps delivering.
pub async fn stop_tail(mut ui: Signal<FeedsUi>, backend: Rc<dyn Backend>) {
    let session = ui.peek().tail.session.clone();
    let Some(session) = session else { return };
    ui.write().tail.stop();
    if let Err(e) = backend.close_feed_session(&session).await {
        ui.write().notice = Some(Notice::err(format!(
            "The tail stopped here, but the server did not confirm closing Studio's consumer group: {e}"
        )));
    }
}

/// Create the feed or save the edit the drawer holds.
pub async fn save_editor(mut ui: Signal<FeedsUi>, backend: Rc<dyn Backend>) {
    let Some(editor) = ui.peek().editor.clone() else {
        return;
    };
    if let Some(e) = ui.write().editor.as_mut() {
        e.busy = true;
        e.error = None;
    }
    let result = match &editor.mode {
        EditorMode::Create => backend.create_feed(&editor.draft).await,
        EditorMode::Edit { id, revision } => {
            backend.update_feed(id, *revision, &editor.draft).await
        }
    };
    let mut w = ui.write();
    match result {
        Ok(feed) => {
            w.notice = Some(Notice::ok(match editor.mode {
                EditorMode::Create => {
                    format!("Created change feed {} ({}).", feed.name, feed.id)
                }
                EditorMode::Edit { .. } => {
                    format!("Saved {} at revision {}.", feed.name, feed.revision)
                }
            }));
            w.selected = Some(feed.id);
            w.selected_event = None;
            w.editor = None;
            w.epoch += 1;
        }
        Err(e) => {
            if let Some(ed) = w.editor.as_mut() {
                ed.busy = false;
                ed.error = Some(e.to_string());
            }
        }
    }
}

/// Pause or resume the server's delivery of one feed.
pub async fn set_paused(
    mut ui: Signal<FeedsUi>,
    backend: Rc<dyn Backend>,
    id: String,
    paused: bool,
) {
    let result = backend.set_feed_paused(&id, paused).await;
    let mut w = ui.write();
    match result {
        Ok(feed) => {
            w.notice = Some(Notice::ok(if paused {
                format!(
                    "Paused delivery of {}. Events accumulate until you resume.",
                    feed.name
                )
            } else {
                format!("Resumed delivery of {}.", feed.name)
            }));
            w.epoch += 1;
        }
        Err(e) => w.notice = Some(Notice::err(e.to_string())),
    }
}

/// Delete one feed with its consumers and checkpoints.
pub async fn delete(mut ui: Signal<FeedsUi>, backend: Rc<dyn Backend>, id: String, name: String) {
    let result = backend.delete_feed(&id).await;
    let tail_session = ui
        .peek()
        .tail
        .is_for(&id)
        .then(|| ui.peek().tail.session.clone())
        .flatten();
    if let (Ok(()), Some(session)) = (&result, tail_session) {
        // The feed is gone, and its Studio group with it.
        let _ = backend.close_feed_session(&session).await;
    }
    let mut w = ui.write();
    match result {
        Ok(()) => {
            w.notice = Some(Notice::ok(format!("Deleted change feed {name}.")));
            w.confirm_delete = false;
            w.selected = None;
            w.selected_event = None;
            if w.tail.is_for(&id) {
                w.tail = Default::default();
            }
            w.epoch += 1;
        }
        Err(e) => {
            w.confirm_delete = false;
            w.notice = Some(Notice::err(e.to_string()));
        }
    }
}
