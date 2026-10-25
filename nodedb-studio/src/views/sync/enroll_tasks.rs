//! The enrollment sheet's reads and writes. Each runs under `spawn_forever`
//! and touches root state only. The token never reaches the screen: the
//! server returns it masked, and the command that carries it goes to the
//! clipboard once.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::clipboard::copy_text;
use crate::models::sync_enroll::EnrollDraft;
use crate::services::backend::Backend;
use crate::state::events::feeds::Notice;
use crate::state::sync::DevicesUi;

/// Read what the principal may offer, then open the sheet on a fresh draft.
pub async fn open(mut ui: Signal<DevicesUi>, backend: Rc<dyn Backend>) {
    match backend.enroll_options().await {
        Ok(options) => ui.write().enroll = Some(EnrollDraft::new(&options)),
        Err(e) => ui.write().notice = Some(Notice::err(e.to_string())),
    }
}

/// Issue the token. A field in error stops it before the server is asked.
pub async fn create(mut ui: Signal<DevicesUi>, backend: Rc<dyn Backend>) {
    let Some(draft) = ui.peek().enroll.clone() else {
        return;
    };
    if draft.busy || draft.issued.is_some() {
        return;
    }
    if draft.name_issue().is_some() {
        if let Some(d) = ui.write().enroll.as_mut() {
            d.tried = true;
        }
        return;
    }
    if let Some(d) = ui.write().enroll.as_mut() {
        d.busy = true;
        d.error = None;
    }
    let result = backend.create_enrollment(&draft.request()).await;
    let mut w = ui.write();
    if let Some(d) = w.enroll.as_mut() {
        d.busy = false;
        match result {
            Ok(issued) => d.issued = Some(issued),
            Err(e) => d.error = Some(e.to_string()),
        }
    }
}

/// Fetch the command once and hand it to the clipboard. It is not rendered.
pub async fn copy_command(mut ui: Signal<DevicesUi>, backend: Rc<dyn Backend>) {
    let Some(id) = ui
        .peek()
        .enroll
        .as_ref()
        .and_then(|d| d.issued.as_ref())
        .map(|i| i.device_id.clone())
    else {
        return;
    };
    let result = match backend.enrollment_command(&id).await {
        Ok(command) => copy_text(command).await.map_err(|e| e.to_string()),
        Err(e) => Err(e.to_string()),
    };
    if let Some(d) = ui.write().enroll.as_mut() {
        match result {
            Ok(()) => {
                d.copied = true;
                d.error = None;
            }
            Err(e) => {
                d.error = Some(format!(
                    "The command did not reach the clipboard: {e}. The server hands it out once, so create a new token."
                ));
            }
        }
    }
}
