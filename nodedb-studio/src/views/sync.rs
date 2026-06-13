//! Sync dashboard view. Phase 3 placeholder; Phase 4 ports the mockup.
use dioxus::prelude::*;
use crate::views::ViewPlaceholder;

#[component]
pub fn Sync() -> Element {
    rsx! { ViewPlaceholder { title: "Sync", note: "CRDT sync dashboard placeholder — Phase 4." } }
}
