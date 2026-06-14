//! Notification popover: capability-filtered, grouped list with a bell badge,
//! mark-all-read, and click-to-navigate.
//!
//! SEAM-04 render-path proof: the rendered list is fetched through the async
//! `ConnectionService` seam via `use_resource` (reading `Rc<dyn ConnectionService>`
//! from context), and the four UI states are surfaced exactly as the
//! `AsyncState`/`AsyncView` primitive (01-03) prescribes — Loading / Empty /
//! Error render via the shared `AsyncView`, Loaded renders the existing grouped
//! list. The Error state offers a Retry gated on `StudioError::is_retriable()`,
//! wired to `Resource::restart()`.
//!
//! Reconciliation with 01-02's app.rs seeding: plan 01-02 seeds a global
//! `Signal<Vec<Notification>>` that `topbar.rs` reads for the unread bell badge.
//! That signal and the topbar badge are LEFT UNCHANGED here — the badge keeps
//! deriving from the app.rs-seeded signal, and mark-all-read / per-item clicks
//! still mutate it so the badge stays in sync. The popover ADDITIONALLY
//! self-fetches via `use_resource` as the live proof of the async pattern. The
//! minor redundancy (badge from signal, list from resource) is intentional and
//! acceptable for Phase 1; later phases converge the two onto one source.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::AsyncView;
use crate::models::notification::{Notification, NotificationTarget};
use crate::routes::Route;
use crate::services::connection_service::ConnectionService;
use crate::state::connection::{ActiveConnection, Capabilities};
use crate::state::notifications::visible;
use crate::state::ui::Popover;

/// Where a notification navigates when clicked.
fn target_route(target: NotificationTarget) -> Route {
    match target {
        NotificationTarget::Sync => Route::Sync {},
        NotificationTarget::StreamsTopics => Route::Streams {
            tab: "topics".to_string(),
        },
        NotificationTarget::StreamsCron => Route::Streams {
            tab: "cron".to_string(),
        },
        NotificationTarget::Admin => Route::Admin {
            tab: "cluster".to_string(),
        },
        NotificationTarget::Query => Route::Query {},
    }
}

#[component]
pub fn NotificationPopover() -> Element {
    // Global signal kept for mark-all-read + per-item read + topbar badge sync.
    let mut notifs = use_context::<Signal<Vec<Notification>>>();
    let mut popover = use_context::<Signal<Option<Popover>>>();
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let service = use_context::<Rc<dyn ConnectionService>>();
    let nav = use_navigator();

    // Capability gate (unchanged): no connection -> render nothing.
    let caps: Capabilities = match active.read().as_ref() {
        Some(c) => c.capabilities,
        None => return rsx! {},
    };

    // SEAM-04 PROOF: fetch the feed through the async seam. Clone the Rc BEFORE
    // the async block; never hold a signal/Resource guard across `.await`.
    let mut feed = use_resource(move || {
        let service = service.clone();
        async move { service.notifications().await } // Result<Vec<Notification>, StudioError>
    });

    // Map the resource read -> the four AsyncState states (mirrors
    // AsyncState::from_value: None->Loading, Some(Err)->Error,
    // Some(Ok(empty))->Empty, Some(Ok(data))->Loaded). StudioError is not Clone,
    // so derive (message, retriable) by reference while the guard is held, clone
    // ONLY the Ok Vec (Notification is Clone), then DROP the guard before render.
    let mut loading = false;
    let mut empty = false;
    let mut error_msg: Option<String> = None;
    let mut retriable = false;
    let mut loaded: Option<Vec<Notification>> = None;
    {
        let read = feed.read();
        match &*read {
            None => loading = true,
            Some(Err(e)) => {
                error_msg = Some(e.to_string()); // Display
                retriable = e.is_retriable(); // gates Retry
            }
            Some(Ok(list)) => {
                // Capability-filter here so Empty reflects what THIS connection sees.
                let vis: Vec<Notification> = visible(&list[..], &caps).cloned().collect();
                if vis.is_empty() {
                    empty = true;
                } else {
                    loaded = Some(vis);
                }
            }
        }
    } // guard dropped here — nothing held across the render below

    // Build the grouped list ONLY for the Loaded case (preserves first-seen order).
    let mut groups: Vec<(String, Vec<Notification>)> = Vec::new();
    let mut unread = 0usize;
    if let Some(ref items) = loaded {
        unread = items.iter().filter(|n| n.unread).count();
        for n in items {
            match groups.iter_mut().find(|(g, _)| g == &n.group) {
                Some((_, gi)) => gi.push(n.clone()),
                None => groups.push((n.group.clone(), vec![n.clone()])),
            }
        }
    }

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
                        // Mark-all-read mutates the GLOBAL signal so the topbar badge updates.
                        for n in notifs.write().iter_mut() { n.unread = false; }
                    },
                    "Mark all read"
                }
            }
            div { class: "notif-list",
                // Loading / Empty / Error -> shared AsyncView (the proof).
                AsyncView {
                    loading,
                    empty,
                    error: error_msg.clone(),
                    retriable,
                    empty_message: "No notifications for this connection".to_string(),
                    on_retry: move |_| feed.restart(),
                }
                // Loaded -> the existing grouped list markup.
                if loaded.is_some() {
                    for (group_name, items) in groups {
                        div { class: "notif-group-label", "{group_name}" }
                        for n in items {
                            {
                                let id = n.id.clone();
                                let route = target_route(n.target);
                                let item_class = if n.unread { "notif-item" } else { "notif-item read" };
                                rsx! {
                                    div {
                                        key: "{id}",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::async_state::AsyncState;
    use crate::services::error::StudioError;

    #[test]
    fn async_state_notifications_none_is_loading() {
        let v: Option<Result<Vec<Notification>, StudioError>> = None;
        assert!(matches!(AsyncState::from_value(v), AsyncState::Loading));
    }

    #[test]
    fn async_state_notifications_empty_is_empty() {
        let v: Option<Result<Vec<Notification>, StudioError>> = Some(Ok(Vec::new()));
        assert!(matches!(AsyncState::from_value(v), AsyncState::Empty));
    }

    #[test]
    fn async_state_notifications_err_is_error() {
        let v: Option<Result<Vec<Notification>, StudioError>> =
            Some(Err(StudioError::NotConnected));
        assert!(matches!(AsyncState::from_value(v), AsyncState::Error(_)));
    }
}
