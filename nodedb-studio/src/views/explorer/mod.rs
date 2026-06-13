//! Explorer: per-storage-mode collection browser.
//! Phase 3 placeholder; Phase 4 adds the sidebar + per-mode viewers.
use dioxus::prelude::*;
use crate::views::ViewPlaceholder;

#[component]
pub fn Explorer() -> Element {
    rsx! { ViewPlaceholder { title: "Explorer", note: "Engine-aware explorer placeholder — Phase 4." } }
}
