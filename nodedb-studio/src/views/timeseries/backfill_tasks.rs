//! Backfill writes. Each runs under `spawn_forever` and touches root state
//! only, so a run request finishes when the user leaves the page.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::services::backend::Backend;
use crate::state::timeseries::{BackfillUi, Notice, PreviewState};
use crate::views::timeseries::backfill_ops::valid_preview;

/// Run the dry run for the current form. A reply that arrives after a newer
/// dry run is dropped.
pub async fn dry_run(mut ui: Signal<BackfillUi>, backend: Rc<dyn Backend>) {
    let spec = ui.peek().spec.clone();
    let seq = {
        let mut w = ui.write();
        w.seq += 1;
        w.preview = PreviewState::Running(w.seq);
        w.error = None;
        w.seq
    };
    let result = backend.backfill_preview(&spec).await;
    let mut w = ui.write();
    if w.preview != PreviewState::Running(seq) {
        return;
    }
    w.preview = match result {
        Ok(p) => PreviewState::Ready(p),
        Err(e) => PreviewState::Failed(e.to_string()),
    };
}

/// Start the job under the reviewed preview and its idempotency key.
pub async fn run(mut ui: Signal<BackfillUi>, backend: Rc<dyn Backend>) {
    let (spec, revision) = {
        let r = ui.peek();
        let Some(p) = valid_preview(&r) else {
            return;
        };
        (r.spec.clone(), p.revision.clone())
    };
    let key = spec.idempotency_key(&revision);
    {
        let mut w = ui.write();
        w.busy = true;
        w.error = None;
    }
    let result = backend.start_backfill(&spec, &revision, &key).await;
    let mut w = ui.write();
    w.busy = false;
    match result {
        Ok(job) => {
            w.review = None;
            w.epoch += 1;
            w.notice = Some(Notice::ok(format!(
                "{} {}: {} rows. Checkpoint recorded under {}.",
                job.id,
                job.state.label().to_lowercase(),
                crate::models::format::thousands(job.rows),
                job.idempotency_key
            )));
        }
        Err(e) => w.error = Some(e.to_string()),
    }
}

/// Continue an interrupted job from its checkpoint.
pub async fn resume(mut ui: Signal<BackfillUi>, backend: Rc<dyn Backend>, id: String) {
    let result = backend.resume_backfill(&id).await;
    let mut w = ui.write();
    match result {
        Ok(job) => {
            w.epoch += 1;
            w.notice = Some(Notice::ok(format!(
                "Resumed {} from its checkpoint. It {} with {} rows, none read twice.",
                job.id,
                job.state.label().to_lowercase(),
                crate::models::format::thousands(job.rows)
            )));
        }
        Err(e) => w.notice = Some(Notice::err(e.to_string())),
    }
}
