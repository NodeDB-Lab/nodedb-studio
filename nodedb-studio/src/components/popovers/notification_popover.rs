//! Notification popover: capability-filtered, grouped list with a bell badge,
//! mark-all-read, and click-to-navigate.
//!
//! The rendered list is fetched through the async `ConnectionService` seam via
//! `use_resource`, then mapped to `AsyncState` and rendered through the shared
//! `AsyncView` — the canonical async-UI pattern. Capability filtering is applied
//! to the `Ok` payload *before* `AsyncState::from_value`, so `Empty` reflects
//! what THIS connection can see (not the raw feed). The Error state offers a
//! Retry gated on `StudioError::is_retriable()`, wired to `Resource::restart()`.
//!
//! The topbar bell badge derives from a separate global `Signal<Vec<Notification>>`
//! seeded in `app.rs`; mark-all-read / per-item clicks mutate that signal so the
//! badge stays in sync. Converging the badge and this list onto one source is
//! deferred to a later phase.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::AsyncView;
use crate::models::notification::{Notification, NotificationTarget};
use crate::routes::Route;
use crate::services::async_state::AsyncState;
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

    // Fetch the feed through the async seam. Clone the Rc BEFORE the async block;
    // never hold a signal/Resource guard across `.await`.
    let mut feed = use_resource(move || {
        let service = service.clone();
        async move { service.notifications().await } // Result<Vec<Notification>, StudioError>
    });

    // Clone the resource value out of its guard (StudioError is Clone), mapping
    // the Ok payload to the capability-filtered list so `Empty` reflects what THIS
    // connection sees — then derive every UI state from one `AsyncState`. This is
    // the same `from_value` mapping the unit tests exercise; the render path and
    // the tests share one code path.
    let state: AsyncState<Vec<Notification>> = {
        let mapped = feed
            .read()
            .clone()
            .map(|res| res.map(|list| visible(&list[..], &caps).cloned().collect::<Vec<_>>()));
        AsyncState::from_value(mapped)
    }; // guard dropped here — nothing held across the render below

    // Build the grouped list ONLY for the Loaded case (preserves first-seen order).
    let mut groups: Vec<(String, Vec<Notification>)> = Vec::new();
    let mut unread = 0usize;
    if let Some(items) = state.loaded() {
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
                // Loading / Empty / Error -> shared AsyncView, driven by AsyncState.
                AsyncView {
                    loading: state.is_loading(),
                    empty: state.is_empty(),
                    error: state.error_message(),
                    retriable: state.is_retriable(),
                    empty_message: "No notifications for this connection".to_string(),
                    on_retry: move |_| feed.restart(),
                }
                // Loaded -> the existing grouped list markup.
                if !groups.is_empty() {
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
