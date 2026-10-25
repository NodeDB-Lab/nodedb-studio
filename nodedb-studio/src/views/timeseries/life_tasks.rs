//! Lifecycle writes. Each runs under `spawn_forever` and touches root state
//! only.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::services::backend::Backend;
use crate::services::error::StudioError;
use crate::state::timeseries::{LifeSheet, LifeUi, Notice};

/// Open the review of the pending changes and read it. The review is bound
/// to the catalog revision it reads.
pub async fn open_review(mut life: Signal<LifeUi>, backend: Rc<dyn Backend>, collection: String) {
    let changes = life.peek().pending.clone();
    life.write().sheet = Some(LifeSheet::Review {
        review: None,
        busy: true,
        error: None,
    });
    let result = backend.review_lifecycle(&collection, &changes).await;
    let mut w = life.write();
    // The sheet may have been closed while the review loaded.
    if !matches!(w.sheet, Some(LifeSheet::Review { .. })) {
        return;
    }
    w.sheet = Some(match result {
        Ok(review) => LifeSheet::Review {
            review: Some(review),
            busy: false,
            error: None,
        },
        Err(e) => LifeSheet::Review {
            review: None,
            busy: false,
            error: Some(e.to_string()),
        },
    });
}

/// Apply the pending changes under the revision the review read. A catalog
/// that moved since is refused, and the review is read again: the impact it
/// showed no longer holds.
pub async fn apply(
    mut life: Signal<LifeUi>,
    backend: Rc<dyn Backend>,
    collection: String,
    reviewed: u64,
) {
    let changes = life.peek().pending.clone();
    if let Some(LifeSheet::Review { busy, error, .. }) = life.write().sheet.as_mut() {
        *busy = true;
        *error = None;
    }
    let result = backend
        .apply_lifecycle(&collection, &changes, reviewed)
        .await;
    match result {
        Ok(receipt) => {
            let mut w = life.write();
            w.pending.clear();
            w.sheet = None;
            w.epoch += 1;
            w.notice = Some(Notice::ok(format!(
                "Applied {} change{} as {}. Lifecycle revision {}.",
                receipt.applied,
                if receipt.applied == 1 { "" } else { "s" },
                receipt.operation,
                receipt.revision
            )));
        }
        Err(e @ StudioError::StaleRevision { .. }) => {
            life.write().epoch += 1;
            open_review(life, backend, collection).await;
            if let Some(LifeSheet::Review { error, .. }) = life.write().sheet.as_mut() {
                *error = Some(format!("{e} The review below is read again."));
            }
        }
        Err(e) => {
            if let Some(LifeSheet::Review { busy, error, .. }) = life.write().sheet.as_mut() {
                *busy = false;
                *error = Some(e.to_string());
            }
        }
    }
}

/// Queue a refresh of one rollup. The receipt names the operation: the
/// refresh runs later and its progress is not reported.
pub async fn refresh_now(
    mut life: Signal<LifeUi>,
    backend: Rc<dyn Backend>,
    collection: String,
    rollup: String,
) {
    let result = backend.refresh_rollup(&collection, &rollup).await;
    let mut w = life.write();
    match result {
        Ok(receipt) => {
            w.epoch += 1;
            w.notice = Some(Notice::ok(format!(
                "Queued a refresh of {rollup} as {}. Its progress is not reported.",
                receipt.operation
            )));
        }
        Err(e) => w.notice = Some(Notice::err(e.to_string())),
    }
}
