//! Notification popover: capability-filtered, grouped list with a bell badge,
//! mark-all-read, and click-to-navigate.

use dioxus::prelude::*;

use crate::models::notification::{Notification, NotificationTarget};
use crate::routes::Route;
use crate::state::connection::{ActiveConnection, Capabilities};
use crate::state::notifications::visible;
use crate::state::ui::Popover;

/// Where a notification navigates when clicked.
fn target_route(target: NotificationTarget) -> Route {
    match target {
        NotificationTarget::Sync => Route::Sync {},
        NotificationTarget::StreamsTopics => Route::Streams { tab: "topics".to_string() },
        NotificationTarget::StreamsCron => Route::Streams { tab: "cron".to_string() },
        NotificationTarget::Admin => Route::Admin { tab: "cluster".to_string() },
        NotificationTarget::Query => Route::Query {},
    }
}

#[component]
pub fn NotificationPopover() -> Element {
    let mut notifs = use_context::<Signal<Vec<Notification>>>();
    let mut popover = use_context::<Signal<Option<Popover>>>();
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let nav = use_navigator();

    let caps: Capabilities = match active.read().as_ref() {
        Some(c) => c.capabilities,
        None => return rsx! {},
    };

    // Capability-filtered view, grouped by `group` preserving first-seen order.
    let list = notifs.read();
    let visible_items: Vec<Notification> = visible(&list[..], &caps).cloned().collect();
    let unread = visible_items.iter().filter(|n| n.unread).count();
    let mut groups: Vec<(String, Vec<Notification>)> = Vec::new();
    for n in &visible_items {
        match groups.iter_mut().find(|(g, _)| g == &n.group) {
            Some((_, items)) => items.push(n.clone()),
            None => groups.push((n.group.clone(), vec![n.clone()])),
        }
    }
    let empty = visible_items.is_empty();
    drop(list);

    let count_label = if unread == 0 {
        "all clear".to_string()
    } else {
        format!("{unread} unread")
    };

    rsx! {
        div { class: "notif-popover open", onclick: move |e| e.stop_propagation(),
            div { class: "notif-header",
                h4 { "Notifications " span { class: "count", "{count_label}" } }
                button {
                    onclick: move |_| {
                        for n in notifs.write().iter_mut() { n.unread = false; }
                    },
                    "Mark all read"
                }
            }
            div { class: "notif-list",
                if empty {
                    div { class: "notif-empty",
                        div { class: "big", "✓" }
                        div { "No notifications for this connection" }
                    }
                } else {
                    for (group_name, items) in groups {
                        div { class: "notif-group-label", "{group_name}" }
                        for n in items {
                            {
                                let id = n.id.clone();
                                let route = target_route(n.target);
                                let item_class = if n.unread { "notif-item" } else { "notif-item read" };
                                rsx! {
                                    div {
                                        class: "{item_class}",
                                        onclick: move |_| {
                                            for x in notifs.write().iter_mut() {
                                                if x.id == id { x.unread = false; }
                                            }
                                            popover.set(None);
                                            nav.push(route.clone());
                                        },
                                        span { class: "sev {n.severity.css_class()}" }
                                        div { class: "body",
                                            div { class: "title", "{n.title}" }
                                            div { class: "desc", "{n.desc}" }
                                        }
                                        span { class: "when", "{n.when}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "notif-footer",
                a { "Settings" }
                a { "View all" }
            }
        }
    }
}
