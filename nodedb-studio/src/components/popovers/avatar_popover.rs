//! Avatar / identity popover. Identity is per-connection: the letter,
//! username, and "role on <connection>" all come from the active connection.

use dioxus::prelude::*;

use crate::state::connection::ActiveConnection;
use crate::state::ui::{ModalKind, Popover};

#[component]
pub fn AvatarPopover() -> Element {
    let mut active = use_context::<Signal<Option<ActiveConnection>>>();
    let mut popover = use_context::<Signal<Option<Popover>>>();
    let mut modal = use_context::<Signal<Option<ModalKind>>>();

    let conn = active.read();
    let Some(c) = conn.as_ref() else {
        return rsx! {};
    };
    let letter = c.avatar_letter();
    let user = c.user.clone();
    let role_on = format!("{} on {}", c.role, c.name);

    rsx! {
        div { class: "avatar-popover open", onclick: move |e| e.stop_propagation(),
            div { class: "ap-header",
                div { class: "big-avatar", "{letter}" }
                div { class: "who",
                    div { class: "name", "{user}" }
                    div { class: "role", "{role_on}" }
                }
            }

            div { class: "ap-section-label", "You" }
            div {
                class: "ap-item",
                onclick: move |_| { popover.set(None); modal.set(Some(ModalKind::NewConnection)); },
                "Connection details…"
            }
            div {
                class: "ap-item danger",
                onclick: move |_| active.set(None),
                "Disconnect " span { class: "kbd", "⌘D" }
            }

            div { class: "ap-divider" }

            div { class: "ap-section-label", "App" }
            div {
                class: "ap-item",
                onclick: move |_| { popover.set(None); modal.set(Some(ModalKind::Preferences)); },
                "Preferences… " span { class: "kbd", "⌘," }
            }
            div {
                class: "ap-item",
                onclick: move |_| { popover.set(None); modal.set(Some(ModalKind::Preferences)); },
                "Keyboard shortcuts"
            }
            div {
                class: "ap-item",
                onclick: move |_| { popover.set(None); modal.set(Some(ModalKind::Preferences)); },
                "About NodeDB-Studio"
            }
        }
    }
}
