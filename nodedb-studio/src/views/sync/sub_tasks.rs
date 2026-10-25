//! The Subscriptions tab's writes. Each runs under `spawn_forever` and
//! touches root state only.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::models::sync_sub::ChangePreview;
use crate::services::backend::Backend;
use crate::state::events::feeds::Notice;
use crate::state::sync::{SubTab, SubsUi};

/// Apply the previewed change, against the revision the preview read.
pub async fn apply(mut ui: Signal<SubsUi>, backend: Rc<dyn Backend>, preview: ChangePreview) {
    if ui.peek().busy {
        return;
    }
    {
        let mut w = ui.write();
        w.busy = true;
        w.error = None;
    }
    let result = backend
        .apply_subscription_change(&preview.subscription, &preview.change, preview.revision)
        .await;
    let mut w = ui.write();
    w.busy = false;
    match result {
        Ok(done) => {
            w.notice = Some(Notice::ok(format!(
                "{} Operation {}.",
                done.summary, done.operation
            )));
            w.draft = None;
            w.tab = SubTab::Definition;
            w.epoch += 1;
        }
        Err(e) => w.error = Some(e.to_string()),
    }
}

/// Pause or resume one subscription.
pub async fn set_paused(
    mut ui: Signal<SubsUi>,
    backend: Rc<dyn Backend>,
    id: String,
    paused: bool,
    revision: u64,
) {
    let result = backend.set_subscription_paused(&id, paused, revision).await;
    let mut w = ui.write();
    match result {
        Ok(done) => {
            w.notice = Some(Notice::ok(format!(
                "{} Operation {}.",
                done.summary, done.operation
            )));
            w.epoch += 1;
        }
        Err(e) => w.notice = Some(Notice::err(e.to_string())),
    }
}
