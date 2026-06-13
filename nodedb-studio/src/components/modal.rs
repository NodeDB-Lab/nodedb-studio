//! Generic modal primitive + the modal host.
//!
//! `Modal` is the reusable overlay shell (backdrop, header, close). `ModalHost`
//! decides which modal is open from the shared `ModalKind` signal. The bodies
//! here are Phase-3 placeholders; Phase 5 replaces them with the real
//! `modals::new_connection` and `modals::preferences` content.

use dioxus::prelude::*;

use crate::state::ui::ModalKind;

/// Reusable modal shell. Clicking the backdrop or the close button dismisses;
/// clicks inside the panel do not (matches the mockup's `closeModal`).
#[component]
pub fn Modal(title: String, children: Element) -> Element {
    let mut modal = use_context::<Signal<Option<ModalKind>>>();
    rsx! {
        div {
            class: "modal-overlay open",
            onclick: move |_| modal.set(None),
            div {
                class: "modal",
                onclick: move |e| e.stop_propagation(),
                div { class: "modal-header",
                    h3 { "{title}" }
                    button { class: "top-icon-btn", onclick: move |_| modal.set(None), "×" }
                }
                {children}
            }
        }
    }
}

/// Renders the currently-open modal, or nothing.
#[component]
pub fn ModalHost() -> Element {
    let modal = use_context::<Signal<Option<ModalKind>>>();
    let kind = *modal.read();

    match kind {
        None => rsx! {},
        Some(ModalKind::NewConnection) => rsx! {
            Modal { title: "New connection",
                div { class: "modal-body",
                    // Phase 5 ports the real form here. Per CLAUDE.md the form
                    // must NOT include an Engine picker (single-engine client).
                    p { "Connection form — ported in Phase 5." }
                }
            }
        },
        Some(ModalKind::Preferences) => rsx! {
            Modal { title: "Preferences",
                div { class: "modal-body",
                    p { "Preferences panes (theme, fonts, keyboard, telemetry, about) — ported in Phase 5." }
                }
            }
        },
    }
}
