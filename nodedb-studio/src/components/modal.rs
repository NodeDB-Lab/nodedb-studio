//! Generic dialog primitive: the scrim, the frame, the header with its
//! close button. Callers pass the body and footer as children, in
//! `.dialog-body` and `.dialog-footer`. The dispatch that decides *which*
//! dialog is open lives in `crate::modals`.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::state::ui::ModalKind;

/// Reusable dialog. The close button and Esc dismiss. A click on the scrim
/// dismisses too, unless `dismiss_on_scrim` is off: a form must not lose
/// typed text to a stray click. `width` sets the frame width in px.
#[component]
pub fn Modal(
    title: String,
    #[props(default)] width: Option<u32>,
    #[props(default = true)] dismiss_on_scrim: bool,
    #[props(default)] class: String,
    children: Element,
) -> Element {
    let mut modal = use_context::<Signal<Option<ModalKind>>>();
    let style = width.map(|w| format!("width: {w}px;")).unwrap_or_default();
    rsx! {
        div {
            class: "dialog-scrim",
            onclick: move |_| {
                if dismiss_on_scrim {
                    modal.set(None);
                }
            },
            div {
                class: "dialog {class}",
                style: "{style}",
                role: "dialog",
                "aria-modal": "true",
                "aria-label": "{title}",
                onclick: move |e| e.stop_propagation(),
                header { class: "dialog-header",
                    h2 { class: "dialog-title", "{title}" }
                    button {
                        class: "icon-btn",
                        title: "Close",
                        "aria-label": "Close {title}",
                        onclick: move |_| modal.set(None),
                        Ico { icon: Icon::X, size: 18 }
                    }
                }
                {children}
            }
        }
    }
}
