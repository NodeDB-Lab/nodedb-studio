//! Top bar: connection chip, database chip, read-only pill, search trigger,
//! notification bell, avatar — each opening its popover.
//!
//! Popovers are mutually exclusive via the shared `Signal<Option<Popover>>`.
//! Chip clicks stop propagation; a click anywhere else (handled by the shell)
//! closes whichever popover is open.

use dioxus::prelude::*;

use crate::components::popovers::avatar_popover::AvatarPopover;
use crate::components::popovers::connection_popover::ConnectionPopover;
use crate::components::popovers::database_popover::DatabasePopover;
use crate::components::popovers::notification_popover::NotificationPopover;
use crate::models::notification::Notification;
use crate::state::connection::ActiveConnection;
use crate::state::notifications::unread_count;
use crate::state::ui::Popover;

/// Open `which`, or close it if it is already the open popover.
fn toggled(current: Option<Popover>, which: Popover) -> Option<Popover> {
    if current == Some(which) { None } else { Some(which) }
}

#[component]
pub fn Topbar() -> Element {
    let mut popover = use_context::<Signal<Option<Popover>>>();
    let mut palette = use_context::<Signal<bool>>();
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let notifs = use_context::<Signal<Vec<Notification>>>();

    let conn = active.read();
    let Some(c) = conn.as_ref() else {
        return rsx! {};
    };

    let badge = unread_count(&notifs.read()[..], &c.capabilities);
    let badge_class = if badge == 0 { "bell-badge zero" } else { "bell-badge" };

    rsx! {
        header { class: "topbar",
            // Connection chip
            div {
                class: "conn-chip",
                onclick: move |e| { e.stop_propagation(); let cur = *popover.read(); popover.set(toggled(cur, Popover::Connection)); },
                span { class: "dot" }
                span { class: "name", "{c.name}" }
                span { class: "engine", "{c.sub}" }
                span { class: "chevron", "▾" }
                if *popover.read() == Some(Popover::Connection) {
                    ConnectionPopover {}
                }
            }

            // Database chip
            div {
                class: "db-chip",
                onclick: move |e| { e.stop_propagation(); let cur = *popover.read(); popover.set(toggled(cur, Popover::Database)); },
                span { class: "ico", "db" }
                span { class: "name", "{c.current_database}" }
                span { class: "chevron", "▾" }
                if *popover.read() == Some(Popover::Database) {
                    DatabasePopover {}
                }
            }

            if c.capabilities.readonly {
                span { class: "readonly-pill", span { class: "dot" } "READ-ONLY" }
            }

            div { class: "topbar-spacer" }

            button {
                class: "search-trigger",
                onclick: move |e| { e.stop_propagation(); palette.set(true); },
                span { "Search or run command…" }
                span { class: "kbd", "⌘K" }
            }

            // Notification bell
            div {
                class: "top-icon-btn bell-btn",
                onclick: move |e| { e.stop_propagation(); let cur = *popover.read(); popover.set(toggled(cur, Popover::Notifications)); },
                svg { width: "14", height: "14", view_box: "0 0 16 16", fill: "none", stroke: "currentColor", stroke_width: "1.5",
                    path { d: "M4 6a4 4 0 0 1 8 0v3l1 2H3l1-2zM6 13a2 2 0 0 0 4 0" }
                }
                span { class: "{badge_class}", "{badge}" }
                if *popover.read() == Some(Popover::Notifications) {
                    NotificationPopover {}
                }
            }

            // Avatar
            div {
                class: "avatar",
                onclick: move |e| { e.stop_propagation(); let cur = *popover.read(); popover.set(toggled(cur, Popover::Avatar)); },
                span { "{c.avatar_letter()}" }
                if *popover.read() == Some(Popover::Avatar) {
                    AvatarPopover {}
                }
            }
        }
    }
}
