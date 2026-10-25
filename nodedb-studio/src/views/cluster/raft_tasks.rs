//! The Raft groups tab's writes. Each runs under `spawn_forever` and touches
//! root state only.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::services::backend::Backend;
use crate::state::cluster::{RaftUi, TransferReview};
use crate::state::events::feeds::Notice;

/// Read the transfer preview and open the review. Nothing changes yet.
pub async fn open_transfer(
    mut ui: Signal<RaftUi>,
    backend: Rc<dyn Backend>,
    group: String,
    target_id: String,
) {
    match backend
        .preview_leadership_transfer(&group, &target_id)
        .await
    {
        Ok(preview) => {
            ui.write().review = Some(TransferReview {
                preview,
                busy: false,
                error: None,
            });
        }
        Err(e) => ui.write().notice = Some(Notice::err(e.to_string())),
    }
}

/// Apply the previewed transfer, against the revision the preview read.
pub async fn apply_transfer(mut ui: Signal<RaftUi>, backend: Rc<dyn Backend>) {
    let Some(review) = ui.peek().review.clone() else {
        return;
    };
    if let Some(r) = ui.write().review.as_mut() {
        r.busy = true;
        r.error = None;
    }
    let p = &review.preview;
    let result = backend
        .transfer_leadership(&p.group, &p.to_id, p.revision)
        .await;
    let mut w = ui.write();
    match result {
        Ok(done) => {
            w.notice = Some(Notice::ok(format!(
                "{} Operation {}.",
                done.summary, done.operation
            )));
            w.review = None;
            w.target = None;
            w.epoch += 1;
        }
        Err(e) => {
            if let Some(r) = w.review.as_mut() {
                r.busy = false;
                r.error = Some(e.to_string());
            }
        }
    }
}

/// Snapshot one group.
pub async fn snapshot(mut ui: Signal<RaftUi>, backend: Rc<dyn Backend>, group: String) {
    ui.write().snapshotting = true;
    let result = backend.create_snapshot(&group).await;
    let mut w = ui.write();
    w.snapshotting = false;
    match result {
        Ok(snap) => {
            w.notice = Some(Notice::ok(format!(
                "Created snapshot {} of {group} at index {}.",
                snap.id, snap.index
            )));
            w.epoch += 1;
        }
        Err(e) => w.notice = Some(Notice::err(e.to_string())),
    }
}
