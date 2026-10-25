//! The notifications page's writes. Each runs under `spawn_forever` and
//! touches root state only.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::services::backend::Backend;
use crate::state::events::feeds::Notice;
use crate::state::events::notify::NotifyUi;

/// Start Studio's listening session on the chosen channel.
pub async fn start(mut ui: Signal<NotifyUi>, backend: Rc<dyn Backend>) {
    let channel = ui.peek().channel.clone();
    match backend.start_listening(&channel).await {
        Ok(session) => {
            let mut w = ui.write();
            w.session = Some(session);
            w.paused = false;
            w.error = None;
        }
        Err(e) => ui.write().error = Some(format!("Could not start listening on {channel}: {e}")),
    }
}

/// End the session. The display keeps what it shows.
pub async fn stop(mut ui: Signal<NotifyUi>, backend: Rc<dyn Backend>) {
    let session = ui.peek().session.clone();
    let Some(session) = session else { return };
    {
        let mut w = ui.write();
        w.session = None;
        w.resume();
    }
    if let Err(e) = backend.stop_listening(&session).await {
        ui.write().error = Some(format!(
            "Studio stopped showing the channel, but the server did not confirm the end of session {}: {e}",
            session.id
        ));
    }
}

/// Send the payload in the Send notification panel.
pub async fn send(mut ui: Signal<NotifyUi>, backend: Rc<dyn Backend>) {
    let (channel, payload) = {
        let u = ui.peek();
        (u.send_channel.clone(), u.payload.clone())
    };
    ui.write().sending = true;
    let result = backend.send_notification(&channel, &payload).await;
    let mut w = ui.write();
    w.sending = false;
    w.sent = Some(match result {
        Ok(0) => Notice::ok(format!(
            "Sent to {channel}. No session was listening, so nobody received it. A notification is not kept."
        )),
        Ok(n) => Notice::ok(format!(
            "Sent to {channel}. {n} listening session{} received it.",
            if n == 1 { "" } else { "s" }
        )),
        Err(e) => Notice::err(format!("Could not send to {channel}: {e}")),
    });
}
