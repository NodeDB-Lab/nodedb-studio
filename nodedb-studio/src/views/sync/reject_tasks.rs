//! The Rejected writes tab's reads and writes. Each runs under
//! `spawn_forever` and touches root state only.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::models::record_json::{parse_object, pretty};
use crate::models::schema::SchemaField;
use crate::models::sync_reject::Rejection;
use crate::services::backend::Backend;
use crate::state::events::feeds::Notice;
use crate::state::sync::{RejectUi, RepairSheet};

/// The fields of `collection`, or none when the Explorer does not hold it.
/// Fields order the JSON and type its values.
async fn fields_of(backend: &Rc<dyn Backend>, collection: &str) -> Vec<SchemaField> {
    backend
        .collection_schema(collection)
        .await
        .map(|s| s.fields)
        .unwrap_or_default()
}

/// Open the edit sheet on the rejected payload.
pub async fn open_edit(mut ui: Signal<RejectUi>, backend: Rc<dyn Backend>, r: Rejection) {
    let fields = fields_of(&backend, &r.collection).await;
    ui.write().sheet = Some(RepairSheet::Edit {
        mutation: r.mutation.clone(),
        revision: r.revision,
        text: pretty(&r.payload, &fields),
        busy: false,
        error: None,
    });
}

/// Submit the edited payload as a new write.
pub async fn resubmit(mut ui: Signal<RejectUi>, backend: Rc<dyn Backend>, collection: String) {
    let Some(RepairSheet::Edit {
        mutation,
        revision,
        text,
        busy,
        ..
    }) = ui.peek().sheet.clone()
    else {
        return;
    };
    if busy {
        return;
    }
    let fields = fields_of(&backend, &collection).await;
    let payload = match parse_object(&text, &fields) {
        Ok(v) => v,
        Err(e) => {
            set_error(ui, e.message);
            return;
        }
    };
    set_busy(ui, true);
    let result = backend
        .resubmit_rejected(&mutation, &payload, revision)
        .await;
    finish(
        &mut ui,
        result.map(|r| format!("{} Operation {}.", r.summary, r.operation)),
    );
}

/// Tell the device to drop its pending copy.
pub async fn discard(mut ui: Signal<RejectUi>, backend: Rc<dyn Backend>) {
    let Some(RepairSheet::Discard {
        mutation,
        revision,
        busy,
        ..
    }) = ui.peek().sheet.clone()
    else {
        return;
    };
    if busy {
        return;
    }
    set_busy(ui, true);
    let result = backend.discard_rejected(&mutation, revision).await;
    finish(
        &mut ui,
        result.map(|r| format!("{} Operation {}.", r.summary, r.operation)),
    );
}

/// Apply the write again, unchanged.
pub async fn retry(
    mut ui: Signal<RejectUi>,
    backend: Rc<dyn Backend>,
    mutation: String,
    revision: u64,
) {
    let result = backend.retry_rejected(&mutation, revision).await;
    let mut w = ui.write();
    match result {
        Ok(r) => {
            w.notice = Some(Notice::ok(format!(
                "{} Operation {}.",
                r.summary, r.operation
            )));
            w.epoch += 1;
        }
        Err(e) => w.notice = Some(Notice::err(e.to_string())),
    }
}

fn set_busy(mut ui: Signal<RejectUi>, on: bool) {
    match ui.write().sheet.as_mut() {
        Some(RepairSheet::Edit { busy, error, .. } | RepairSheet::Discard { busy, error, .. }) => {
            *busy = on;
            *error = None;
        }
        Some(RepairSheet::Compensation { .. }) | None => {}
    }
}

fn set_error(mut ui: Signal<RejectUi>, message: String) {
    match ui.write().sheet.as_mut() {
        Some(RepairSheet::Edit { busy, error, .. } | RepairSheet::Discard { busy, error, .. }) => {
            *busy = false;
            *error = Some(message);
        }
        Some(RepairSheet::Compensation { .. }) | None => {}
    }
}

/// Close the sheet on success, keep it open with the error on failure.
fn finish(ui: &mut Signal<RejectUi>, result: Result<String, crate::services::error::StudioError>) {
    match result {
        Ok(text) => {
            let mut w = ui.write();
            w.sheet = None;
            w.notice = Some(Notice::ok(text));
            w.epoch += 1;
        }
        Err(e) => set_error(*ui, e.to_string()),
    }
}
