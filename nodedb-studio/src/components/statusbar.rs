//! Bottom status bar. Reflects the active connection's identity (name, current
//! database, role) — all per-connection, swapping when the connection changes.

use dioxus::prelude::*;

use crate::state::connection::ActiveConnection;

#[component]
pub fn Statusbar() -> Element {
    let active = use_context::<Signal<Option<ActiveConnection>>>();
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
            span { class: "status-item", "⌘K for command palette" }
        }
    }
}
