//! The account menu. Identity is per-connection: the name, account, and
//! role come from the active connection's profile.
//!
//! "Copy connection info" copies where and as whom, never a secret.

use dioxus::prelude::*;

use crate::components::clipboard::copy_text;
use crate::components::icon::{Ico, Icon};
use crate::models::connections::ConnectionKind;
use crate::models::shell::APP_VERSION;
use crate::state::connection::ActiveConnection;
use crate::state::connections_registry::SavedConnection;
use crate::state::ui::{ModalKind, Popover, PrefsPane};

#[component]
pub fn AvatarPopover() -> Element {
    let mut active = use_context::<Signal<Option<ActiveConnection>>>();
    let mut popover = use_context::<Signal<Option<Popover>>>();
    let mut modal = use_context::<Signal<Option<ModalKind>>>();
    let registry = use_context::<Signal<Vec<SavedConnection>>>();
    // The text waiting to be copied. The resource copies it and reports, so
    // the button can say whether the clipboard took it.
    let mut to_copy = use_signal(|| None::<String>);
    let copied = use_resource(move || async move {
        let text = to_copy.read().clone()?;
        Some(copy_text(text).await)
    });

    let Some(c) = active.read().clone() else {
        return rsx! {};
    };
    let saved = registry.read().iter().find(|s| s.name == c.name).cloned();
    let place_label = match saved.as_ref().map(SavedConnection::kind) {
        Some(ConnectionKind::Lite) => "File",
        Some(ConnectionKind::Origin) | None => "Cluster",
    };
    let info = saved
        .as_ref()
        .map(|s| s.info_text(&c.current_database, &c.user))
        .unwrap_or_else(|| {
            format!(
                "{} · database {} · user {}",
                c.name, c.current_database, c.user
            )
        });
    let copy_label = match copied.read().as_ref() {
        Some(Some(Ok(()))) => "Copied",
        Some(Some(Err(_))) => "Copy failed",
        Some(None) | None => "Copy connection info",
    };

    let open_prefs = move |pane: PrefsPane| {
        move |_| {
            popover.set(None);
            modal.set(Some(ModalKind::Preferences(pane)));
        }
    };

    rsx! {
        div {
            class: "popover account-menu",
            role: "menu",
            "aria-label": "Account",
            onclick: move |e| e.stop_propagation(),
            div { class: "account-head",
                div { class: "account-avatar", "{c.avatar_letter()}" }
                div { class: "account-who",
                    div { class: "account-name", "{c.display_name}" }
                    div { class: "account-sub", "{c.account()}" }
                    div { class: "account-sub", "{c.role}" }
                }
            }
            div { class: "menu-sep" }
            button { class: "menu-item", role: "menuitem", onclick: open_prefs(PrefsPane::Appearance),
                Ico { icon: Icon::Settings, size: 18 }
                span { "Preferences" }
            }
            button { class: "menu-item", role: "menuitem", onclick: open_prefs(PrefsPane::Keyboard),
                Ico { icon: Icon::Keyboard, size: 18 }
                span { "Keyboard shortcuts" }
            }
            button { class: "menu-item", role: "menuitem", onclick: open_prefs(PrefsPane::About),
                Ico { icon: Icon::Info, size: 18 }
                span { "About NodeDB Studio" }
            }
            div { class: "menu-sep" }
            div { class: "menu-heading", "Connection details" }
            div { class: "kv compact",
                div { class: "kv-row", span { "{place_label}" } span { "{c.name}" } }
                div { class: "kv-row", span { "Database" } span { "{c.current_database}" } }
            }
            div { class: "account-actions",
                button {
                    class: "btn block",
                    r#type: "button",
                    onclick: move |_| to_copy.set(Some(info.clone())),
                    Ico { icon: Icon::Copy, size: 16 }
                    "{copy_label}"
                }
                button {
                    class: "btn danger block",
                    r#type: "button",
                    onclick: move |_| { popover.set(None); active.set(None); },
                    Ico { icon: Icon::Unplug, size: 16 }
                    "Disconnect"
                }
            }
            div { class: "menu-sep" }
            div { class: "account-foot",
                div { class: "kv-row", span { "Version" } span { "NodeDB Studio {APP_VERSION}" } }
                button {
                    class: "link-btn",
                    onclick: move |_| {
                        popover.set(None);
                        modal.set(Some(ModalKind::Preferences(PrefsPane::About)));
                    },
                    "Version details"
                    Ico { icon: Icon::ExternalLink, size: 14 }
                }
            }
        }
    }
}
