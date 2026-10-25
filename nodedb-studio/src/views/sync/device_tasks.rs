//! The Devices tab's writes. Each runs under `spawn_forever` and touches
//! root state only.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::services::backend::Backend;
use crate::state::events::feeds::Notice;
use crate::state::sync::DevicesUi;

/// Pause or resume one device. `revision` is the one the list read.
pub async fn set_paused(
    mut ui: Signal<DevicesUi>,
    backend: Rc<dyn Backend>,
    id: String,
    paused: bool,
    revision: u64,
) {
    let result = backend.set_device_paused(&id, paused, revision).await;
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
