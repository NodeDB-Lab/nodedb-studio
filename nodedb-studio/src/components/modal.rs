//! Generic modal primitive: the reusable overlay shell (backdrop, panel,
//! header, close). Callers pass the body and footer as children. The dispatch
//! that decides *which* modal is open lives in `crate::modals`.

use dioxus::prelude::*;

use crate::state::ui::ModalKind;

/// Reusable modal shell. Clicking the backdrop or the close button dismisses;
/// clicks inside the panel do not (matches the mockup's `closeModal`).
/// `wide` selects the `.modal.wide` variant used by Preferences. `width`
/// overrides the panel width in px, as the mockup's entity forms do.
#[component]
pub fn Modal(
    title: String,
    #[props(default = false)] wide: bool,
    #[props(default)] width: Option<u32>,
    children: Element,
) -> Element {
    let mut modal = use_context::<Signal<Option<ModalKind>>>();
    let panel_class = if wide { "modal wide" } else { "modal" };
    let panel_style = width.map(|w| format!("width: {w}px;")).unwrap_or_default();
    rsx! {
        div {
            class: "modal-overlay open",
            onclick: move |_| modal.set(None),
            div {
                class: "{panel_class}",
                style: "{panel_style}",
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
