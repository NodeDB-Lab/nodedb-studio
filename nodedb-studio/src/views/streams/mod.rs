//! Streams & Events. The `tab` selects the sub-screen (landing/cdc/mv/topics/
//! notify/cron); Phase 4 ports the lateral nav + each sub-screen.
use dioxus::prelude::*;
use crate::views::ViewPlaceholder;

#[component]
pub fn Streams(tab: String) -> Element {
    rsx! { ViewPlaceholder { title: "Streams & Events", note: "Streams sub-screen \"{tab}\" placeholder — Phase 4." } }
}
