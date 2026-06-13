//! The connected state: the studio shell.
//!
//! Provides the studio-scoped UI signals (which popover is open, whether the
//! command palette is open) and mounts the router. The router renders
//! `StudioLayout`, which consumes these signals. `App` mounts this only while
//! connected, so the router never coexists with the Connection Manager.

use dioxus::prelude::*;

use crate::routes::Route;
use crate::state::ui::Popover;

#[component]
pub fn Studio() -> Element {
    // Shared, studio-only UI state. Provided above the router so the layout and
    // topbar (rendered inside the router) can consume them via context.
    use_context_provider(|| Signal::new(None::<Popover>));
    use_context_provider(|| Signal::new(false)); // command palette open

    rsx! {
        Router::<Route> {}
    }
}
