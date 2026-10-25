//! The writes every Administration tab shares: read a review, apply it,
//! close it. Each runs under `spawn_forever` and touches root state only.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::models::admin::AdminChange;
use crate::services::backend::Backend;
use crate::state::admin::{ReviewState, Reviewing};
use crate::state::events::feeds::Notice;

/// Read what `change` would do and open its review. Nothing changes yet.
pub async fn open_review<T: Reviewing + 'static>(
    mut ui: Signal<T>,
    backend: Rc<dyn Backend>,
    change: AdminChange,
) {
    match backend.preview_admin_change(&change).await {
        Ok(review) => *ui.write().review_mut() = Some(ReviewState::new(change, review)),
        Err(e) => *ui.write().notice_mut() = Some(Notice::err(e.to_string())),
    }
}

/// Apply the open review against the revision it read. A failure keeps the
/// review open with the reason, so the person can go back and read again.
pub async fn apply_review<T: Reviewing + 'static>(mut ui: Signal<T>, backend: Rc<dyn Backend>) {
    let Some(open) = ui.peek().review().clone() else {
        return;
    };
    if let Some(r) = ui.write().review_mut().as_mut() {
        r.busy = true;
        r.error = None;
    }
    let result = backend
        .apply_admin_change(&open.change, open.review.revision)
        .await;
    let mut w = ui.write();
    match result {
        Ok(done) => {
            *w.notice_mut() = Some(Notice::ok(format!(
                "{} Operation {}.",
                done.summary, done.operation
            )));
            *w.review_mut() = None;
            w.applied(&open.change);
        }
        Err(e) => {
            if let Some(r) = w.review_mut().as_mut() {
                r.busy = false;
                r.error = Some(e.to_string());
            }
        }
    }
}
