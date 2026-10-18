//! Bottom status bar. Connection name, current database and role come from
//! the active connection; server version and timezone from the seam's
//! `session_info()`. All per-connection: switching connections re-reads them.
//!
//! The mockup's "last query" latency is not shown yet. Nothing produces one
//! until the Query workbench runs through the seam.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::services::backend::Backend;
use crate::state::connection::ActiveConnection;

#[component]
pub fn Statusbar() -> Element {
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let backend = use_context::<Rc<dyn Backend>>();
    let conn_name = use_context::<Memo<Option<String>>>();
    let session = use_resource(move || {
        let backend = backend.clone();
        // Reading the connection's name subscribes this resource to it, so a
        // switch re-reads the new server's version and timezone.
        let _conn = conn_name();
        async move { backend.session_info().await }
    });

    let conn = active.read();
    let Some(c) = conn.as_ref() else {
        return rsx! {};
    };

    rsx! {
        footer { class: "statusbar",
            span { class: "status-item ok", "● connected · {c.name}" }
            span { class: "status-item", "{c.current_database}" }
            span { class: "status-item", "{c.role}" }
            span { class: "spacer" }
            match &*session.read() {
                Some(Ok(s)) => rsx! {
                    span { class: "status-item", "{s.timezone}" }
                    span { class: "status-item", "{s.server_version}" }
                },
                // Said, not hidden: a missing version reads as "no version",
                // which is a different fact from "could not ask".
                Some(Err(e)) => rsx! {
                    span { class: "status-item", title: "{e}", "session info unavailable" }
                },
                None => rsx! {},
            }
            span { class: "status-item", "⌘K for command palette" }
        }
    }
}
