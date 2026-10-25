//! The one place a session opens.
//!
//! The manager, the switcher, the palette, and the connection dialog all
//! open sessions. Each used to carry its own copy of the task that calls the
//! seam and reconciles the result. A copy that forgot to show the error left
//! a Connect button that did nothing. This helper keeps the rules in one
//! spot: the result goes through `apply_connect`, the error goes to the
//! app-wide error bar, and a success closes the manager.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::services::backend::Backend;
use crate::state::connection::{ActiveConnection, ConnectError, apply_connect};
use crate::state::connections_registry::Credentials;
use crate::state::ui::ManagerOpen;

/// Handles for opening a session. Cheap to clone into a handler.
#[derive(Clone)]
pub struct Connector {
    backend: Rc<dyn Backend>,
    active: Signal<Option<ActiveConnection>>,
    error: Signal<ConnectError>,
    manager: Signal<ManagerOpen>,
}

pub fn use_connector() -> Connector {
    Connector {
        backend: use_context(),
        active: use_context(),
        error: use_context(),
        manager: use_context(),
    }
}

impl Connector {
    /// Open the session and report whether it opened. A failure leaves the
    /// current session as it was and shows its error in the error bar.
    pub async fn run(&self, name: &str, creds: &Credentials) -> bool {
        let (mut active, mut error, mut manager) = (self.active, self.error, self.manager);
        // A stale error goes the moment a new attempt starts.
        error.set(ConnectError(None));
        let result = self.backend.connect(name, creds).await;
        let failure = apply_connect(&mut active.write(), result);
        let opened = failure.is_none();
        error.set(ConnectError(failure));
        if opened {
            manager.set(ManagerOpen(false));
        }
        opened
    }

    /// Open the session in a task that outlives the caller. The popover, the
    /// palette, and the manager unmount the moment the attempt starts, and a
    /// scope-bound task would die at its first await.
    pub fn start(&self, name: String, creds: Credentials) {
        let me = self.clone();
        spawn_forever(async move {
            me.run(&name, &creds).await;
        });
    }
}
