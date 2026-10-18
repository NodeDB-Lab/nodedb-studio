//! The connected state: the studio shell.
//!
//! Provides the Explorer selection and mounts the router. The open popover and
//! the palette flag live in `App`, whose root handles every key. `App` mounts
//! this only while connected, so the router never coexists with the
//! Connection Manager.

use dioxus::prelude::*;

use crate::routes::Route;
use crate::state::explorer::Selected;

#[component]
pub fn Studio() -> Element {
    // The Explorer's selected collection, shared with the topbar breadcrumb.
    // Provided above the router so views inside it can consume it.
    use_context_provider(|| Signal::new(None::<Selected>));

    rsx! {
        Router::<Route> {}
    }
}
