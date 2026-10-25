//! The Placement tab's writes. Each runs under `spawn_forever` and touches
//! root state only.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::services::backend::Backend;
use crate::state::cluster::{PlaceUi, PlanReview};
use crate::state::events::feeds::Notice;

/// Read the plan preview and open the review. Nothing moves yet.
pub async fn open_review(mut ui: Signal<PlaceUi>, backend: Rc<dyn Backend>) {
    let staged = ui.peek().staged.clone();
    match backend.preview_rebalance(&staged).await {
        Ok(plan) => {
            ui.write().review = Some(PlanReview {
                plan,
                busy: false,
                error: None,
            });
        }
        Err(e) => ui.write().notice = Some(Notice::err(e.to_string())),
    }
}

/// Start the previewed plan, against the revision the preview read.
pub async fn apply(mut ui: Signal<PlaceUi>, backend: Rc<dyn Backend>) {
    let Some(review) = ui.peek().review.clone() else {
        return;
    };
    if let Some(r) = ui.write().review.as_mut() {
        r.busy = true;
        r.error = None;
    }
    let result = backend.apply_rebalance(&review.plan).await;
    let mut w = ui.write();
    match result {
        Ok(done) => {
            w.notice = Some(Notice::ok(format!(
                "{} Operation {}.",
                done.summary, done.operation
            )));
            w.review = None;
            w.staged.clear();
            w.from = None;
            w.to = None;
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
