//! The header: connection and database selectors on the left, then search,
//! notifications, the View menu, and the avatar grouped on the right.
//!
//! The connection and database appear here once and nowhere else in the
//! chrome. The popovers and the notification drawer are mutually exclusive
//! through the shared `Signal<Option<Popover>>`. Triggers stop propagation; a click anywhere
//! else (handled by the shell) closes the open popover.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::components::popovers::avatar_popover::AvatarPopover;
use crate::components::popovers::connection_popover::ConnectionPopover;
use crate::components::popovers::database_popover::DatabasePopover;
use crate::components::popovers::notification_drawer::NotificationDrawer;
use crate::components::view_menu::ViewMenu;
use crate::models::notification::Notification;
use crate::services::async_state::AsyncState;
use crate::state::connection::ActiveConnection;
use crate::state::notifications::unread_count;
use crate::state::ui::Popover;

/// Open `which`, or close it if it is already the open popover.
fn toggled(current: Option<Popover>, which: Popover) -> Option<Popover> {
    if current == Some(which) {
        None
    } else {
        Some(which)
    }
}

#[component]
pub fn Header() -> Element {
    let mut popover = use_context::<Signal<Option<Popover>>>();
    let mut palette = use_context::<Signal<bool>>();
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let notifs = use_context::<Signal<AsyncState<Vec<Notification>>>>();

    let conn = active.read();
    let Some(c) = conn.as_ref() else {
        return rsx! {};
    };
    let open = *popover.read();
    let mut toggle = move |which: Popover| {
        let cur = *popover.peek();
        popover.set(toggled(cur, which));
    };

    // The capability-filtered unread count over the loaded feed, 0 until it
    // loads.
    let unread = notifs
        .read()
        .loaded()
        .map_or(0, |items| unread_count(items, &c.capabilities));

    rsx! {
        header { class: "header",
            div { class: "header-context",
                div { class: "anchor",
                    button {
                        class: "conn-switch",
                        "aria-haspopup": "menu",
                        "aria-expanded": "{open == Some(Popover::Connection)}",
                        onclick: move |e| { e.stop_propagation(); toggle(Popover::Connection); },
                        span { class: "status-dot ok", "aria-label": "Connected" }
                        span { class: "conn-text",
                            span { class: "conn-name", "{c.name}" }
                            span { class: "conn-sub", "{c.sub}" }
                        }
                        Ico { icon: Icon::ChevronDown, size: 18, class: "chevron" }
                    }
                    if open == Some(Popover::Connection) {
                        ConnectionPopover {}
                    }
                }
                span { class: "header-divider", "aria-hidden": "true" }
                div { class: "anchor",
                    button {
                        class: "db-switch",
                        "aria-haspopup": "menu",
                        "aria-label": "Database {c.current_database}",
                        "aria-expanded": "{open == Some(Popover::Database)}",
                        onclick: move |e| { e.stop_propagation(); toggle(Popover::Database); },
                        Ico { icon: Icon::Database, size: 20 }
                        span { class: "db-name", "{c.current_database}" }
                        Ico { icon: Icon::ChevronDown, size: 18, class: "chevron" }
                    }
                    if open == Some(Popover::Database) {
                        DatabasePopover {}
                    }
                }
            }
            div { class: "header-actions",
                button {
                    class: "search-box",
                    onclick: move |e| { e.stop_propagation(); palette.set(true); },
                    Ico { icon: Icon::Search, size: 18 }
                    span { class: "search-placeholder", "Search or run command…" }
                    kbd { "Cmd K" }
                }
                div { class: "header-icons",
                    div { class: "anchor",
                        button {
                            class: "icon-btn header-icon",
                            title: "Notifications",
                            "aria-label": "Notifications, {unread} unread",
                            "aria-expanded": "{open == Some(Popover::Notifications)}",
                            onclick: move |e| { e.stop_propagation(); toggle(Popover::Notifications); },
                            Ico { icon: Icon::Bell, size: 20 }
                            if unread > 0 {
                                span { class: "count-badge", "{unread}" }
                            }
                        }
                        if open == Some(Popover::Notifications) {
                            NotificationDrawer {}
                        }
                    }
                    div { class: "anchor",
                        button {
                            class: if open == Some(Popover::View) { "icon-btn header-icon pressed" } else { "icon-btn header-icon" },
                            title: "View",
                            "aria-label": "View",
                            "aria-haspopup": "menu",
                            "aria-expanded": "{open == Some(Popover::View)}",
                            onclick: move |e| { e.stop_propagation(); toggle(Popover::View); },
                            Ico { icon: Icon::Columns2, size: 20 }
                        }
                        if open == Some(Popover::View) {
                            ViewMenu {}
                        }
                    }
                    div { class: "anchor",
                        button {
                            class: "avatar",
                            "aria-label": "Account {c.display_name}",
                            "aria-expanded": "{open == Some(Popover::Avatar)}",
                            onclick: move |e| { e.stop_propagation(); toggle(Popover::Avatar); },
                            "{c.avatar_letter()}"
                        }
                        if open == Some(Popover::Avatar) {
                            AvatarPopover {}
                        }
                    }
                }
            }
        }
    }
}
