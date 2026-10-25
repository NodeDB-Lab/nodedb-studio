//! The connected state: the studio shell.
//!
//! Provides the Explorer selection and mounts the router. The open popover and
//! the palette flag live in `App`, whose root handles every key. `App` mounts
//! this only while connected, so the router never coexists with the
//! Connection Manager.

use dioxus::prelude::*;

use crate::routes::Route;
use crate::state::explorer::{RecordView, Selected};

#[component]
pub fn Studio() -> Element {
    // The Explorer's selected collection, shared with the header breadcrumb.
    // Provided above the router so views inside it can consume it.
    let mut selected = use_context_provider(|| Signal::new(None::<Selected>));

    // A selection names a collection on one server. Another server can lack
    // it, or hold one by that name with another profile, so a switch clears
    // it and how it was viewed, and the Explorer picks the new server's
    // default. Only a change clears it: the first run must not undo the
    // Explorer's default.
    let mut view = use_context::<Signal<RecordView>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut seen = use_signal(|| conn.peek().clone());
    use_effect(move || {
        let now = conn();
        if *seen.peek() != now {
            seen.set(now);
            selected.set(None);
            view.set(RecordView::default());
        }
    });

    rsx! {
        Router::<Route> {}
    }
}
