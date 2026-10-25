//! The footer: connection status and session role on the left, the display
//! time zone and the server version on the right. The connection and database names
//! live in the header only.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::resource_state;
use crate::components::icon::{Ico, Icon};
use crate::services::async_state::AsyncState;
use crate::services::backend::Backend;
use crate::state::connection::ActiveConnection;
use crate::state::preferences::Preferences;

#[component]
pub fn Footer() -> Element {
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let backend = use_context::<Rc<dyn Backend>>();
    let conn_name = use_context::<Memo<Option<String>>>();
    let prefs = use_context::<Signal<Preferences>>();
    let session = use_resource(move || {
        let backend = backend.clone();
        // Reading the connection's name re-reads the new server's version
        // on a switch.
        let _conn = conn_name();
        async move { backend.session_info().await }
    });
    let session = resource_state(&session);
    let zone = prefs.read().zone_label();
    let conn = active.read();
    let Some(c) = conn.as_ref() else {
        return rsx! {};
    };

    rsx! {
        footer { class: "footer",
            span { class: "footer-item",
                span { class: "status-dot ok", "aria-hidden": "true" }
                "Connected"
            }
            span { class: "footer-sep", "aria-hidden": "true" }
            span { class: "footer-item", title: "Session role",
                Ico { icon: Icon::User, size: 16 }
                "{c.role}"
            }
            span { class: "footer-spacer" }
            match &session {
                AsyncState::Loaded(s) => rsx! {
                    span { class: "footer-item", title: "Display time zone and server version",
                        "{zone} · {s.server_version}"
                    }
                },
                // Said, not hidden: a missing version reads as "no version",
                // which is a different fact from "the read failed".
                AsyncState::Error(e) => rsx! {
                    span { class: "footer-item warn", title: "{e}", "Session info unavailable" }
                },
                AsyncState::Loading | AsyncState::Empty => rsx! {},
            }
        }
    }
}
