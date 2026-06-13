//! The modal host: renders whichever modal the shared `ModalKind` signal
//! reports as open.

use dioxus::prelude::*;

use crate::components::modal::Modal;
use crate::modals::new_connection::NewConnectionForm;
use crate::modals::preferences::PreferencesPanes;
use crate::state::ui::ModalKind;

/// Renders the currently-open modal (from the shared `ModalKind` signal), or
/// nothing. Overlays everything; provided once near the app root.
#[component]
pub fn ModalHost() -> Element {
    let modal = use_context::<Signal<Option<ModalKind>>>();
    match *modal.read() {
        None => rsx! {},
        Some(ModalKind::NewConnection) => rsx! {
            Modal { title: "New connection", NewConnectionForm {} }
        },
        Some(ModalKind::Preferences) => rsx! {
            Modal { title: "Preferences", wide: true, PreferencesPanes {} }
        },
    }
}
