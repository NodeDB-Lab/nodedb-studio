//! The connected state: the studio shell.
//!
//! Phase 1 renders a minimal placeholder that reflects per-connection identity
//! and proves Disconnect returns to the Connection Manager. The real shell
//! (rail + topbar + statusbar) and the internal `Route` tree land in Phase 3.

use dioxus::prelude::*;

use crate::state::connection::ActiveConnection;

#[component]
pub fn Studio() -> Element {
    let mut active = use_context::<Signal<Option<ActiveConnection>>>();

    // Studio is only mounted by `App` when a connection is active, so the
    // session is always present here.
    let conn = active
        .read()
        .clone()
        .expect("Studio is rendered only while connected");

    rsx! {
        div { class: "studio",
            h1 { "{conn.name}" }
            p { "{conn.user} · {conn.role} · db: {conn.current_database}" }
            button {
                class: "btn",
                onclick: move |_| active.set(None),
                "Disconnect"
            }
        }
    }
}
