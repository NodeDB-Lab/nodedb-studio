//! Notification popover: capability-filtered, grouped list with a bell badge,
//! mark-all-read, and click-to-navigate.
//!
//! Single source of truth: both this popover and the topbar bell badge read the
//! one shared `Signal<AsyncState<Vec<Notification>>>` store (seeded once at the
//! seam in `app.rs`). The popover `project`s that store through capability
//! filtering — so `Empty` reflects what THIS connection sees — then renders the
//! `Loading`/`Empty`/`Error` states through the shared `AsyncView` and the loaded
//! list itself. Mark-all-read / per-item clicks MUTATE the store, so the badge
//! and the list never diverge. The Error-state Retry reloads the feed via the
//! shared `Resource` handle, gated on `StudioError::is_retriable()`.
//!
//! A failed mark-all-read is a WRITE failure and is kept out of the read store:
//! the list the user was looking at is still correct, so it stays on screen and
//! the failure renders beside it, with a retry that re-issues the write.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::AsyncView;
use crate::models::notification::{Notification, NotificationTarget};
use crate::routes::Route;
use crate::services::async_state::AsyncState;
use crate::services::backend::Backend;
use crate::services::error::StudioError;
use crate::state::connection::{ActiveConnection, Capabilities};
use crate::state::notifications::{apply_mark_all_read, mark_read, visible};
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

/// Persists mark-all-read, then reconciles the shared store with the result.
/// The local list changes only once the seam acknowledges the write (see
/// `apply_mark_all_read`), and no signal guard is held across the await.
///
/// Runs under `spawn_forever`: the popover is conditionally mounted, and
/// Dioxus drops a scope's tasks on unmount. Clicking away mid-write would
/// kill the task at the await, leaving the server's state unknown and the
/// store never reconciled. `store` is app-level context and outlives the
/// task, so the reconcile always lands.
///
/// `write_error` belongs to the popover and is dropped when it closes.
/// `Signal::set` on a dropped signal panics, so the error is written only
/// while the popover is open. A failure the user closed the popover on is
/// not shown: the failure path leaves the list untouched, which is what
/// reopening the popover shows.
async fn mark_all_read_and_reconcile(
    backend: Rc<dyn Backend>,
    mut store: Signal<AsyncState<Vec<Notification>>>,
    mut write_error: Signal<Option<StudioError>>,
) {
    let result = backend.mark_all_read().await;
    let err = apply_mark_all_read(&mut store.write(), result);
    if let Ok(mut slot) = write_error.try_write() {
        *slot = err;
    }
}

#[component]
pub fn NotificationPopover() -> Element {
    // The one shared store (mutated here for mark-all-read + per-item read) and
    // the shared reload handle backing the Error-state Retry.
    let mut store = use_context::<Signal<AsyncState<Vec<Notification>>>>();
    let mut reload = use_context::<Resource<()>>();
    let mut popover = use_context::<Signal<Option<Popover>>>();
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let backend = use_context::<Rc<dyn Backend>>();
    let nav = use_navigator();
    // Write failure for mark-all-read, separate from the read store.
    let write_error: Signal<Option<StudioError>> = use_signal(|| None);
    // Built once so the header button and its Retry are the same operation,
    // not two copies that can drift.
    let mark_all = {
        let backend = backend.clone();
        move || {
            spawn_forever(mark_all_read_and_reconcile(
                backend.clone(),
                store,
                write_error,
            ));
        }
    };

    // Capability gate (unchanged): no connection -> render nothing.
    let caps: Capabilities = match active.read().as_ref() {
        Some(c) => c.capabilities,
        None => return rsx! {},
    };

    // Project the shared store through capability filtering into an owned view
    // state: `Empty` re-derives from the filtered list, so it reflects what THIS
    // connection sees. The read guard is dropped at the end of this statement —
    // nothing is held across the render below.
    let view: AsyncState<Vec<Notification>> = store
        .read()
        .project(|raw| visible(raw, &caps).cloned().collect());

    // Build the grouped list ONLY for the Loaded case (preserves first-seen order).
    let mut groups: Vec<(String, Vec<Notification>)> = Vec::new();
    let mut unread = 0usize;
    if let Some(items) = view.loaded() {
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
                    onclick: {
                        let mark_all = mark_all.clone();
                        move |_| mark_all()
                    },
                    "Mark all read"
                }
            }
            // A failed write renders through the same component as a failed
            // read, so it is styled, tested, and gated on retriability exactly
            // once. Its Retry re-issues the write, not the read.
            if let Some(e) = write_error.read().as_ref() {
                AsyncView {
                    loading: false,
                    empty: false,
                    error: Some(format!("Could not mark all read: {e}")),
                    retriable: e.is_retriable(),
                    empty_message: String::new(),
                    on_retry: {
                        let mark_all = mark_all.clone();
                        move |_| mark_all()
                    },
                }
            }
            div { class: "notif-list",
                // Loading / Empty / Error -> shared AsyncView, driven by AsyncState.
                AsyncView {
                    loading: view.is_loading(),
                    empty: view.is_empty(),
                    error: view.error_message(),
                    retriable: view.is_retriable(),
                    empty_message: "No notifications for this connection".to_string(),
                    on_retry: move |_| reload.restart(),
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
                                            if let Some(items) = store.write().loaded_mut() {
                                                mark_read(items, &id);
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

    /// The popover can close while mark-all-read waits on the server. The
    /// result then lands after the popover's own signals are dropped. A mock
    /// in `Ready` resolves on the first poll and never reaches this, so the
    /// write here runs against a backend that yields.
    #[tokio::test]
    async fn mark_all_read_finishing_after_the_popover_closed_does_not_panic() {
        use std::time::Duration;

        use crate::services::connection_service::{ConnectionService, MockConnectionService};

        #[component]
        fn Popover(store: Signal<AsyncState<Vec<Notification>>>) -> Element {
            let backend = use_context::<Rc<dyn Backend>>();
            let write_error = use_signal(|| None);
            use_hook(move || {
                spawn_forever(mark_all_read_and_reconcile(backend, store, write_error));
            });
            rsx! {}
        }

        fn app(mock: Rc<MockConnectionService>) -> Element {
            use_context_provider(|| mock as Rc<dyn Backend>);
            let store = use_signal(|| AsyncState::Loading);
            let mut open = use_signal(|| true);
            // Close the popover while the write is still in flight.
            use_future(move || async move {
                tokio::time::sleep(Duration::from_millis(5)).await;
                open.set(false);
            });
            rsx! {
                if open() {
                    Popover { store }
                }
            }
        }

        let mock = Rc::new(MockConnectionService::delayed(Duration::from_millis(30)));
        let mut dom = VirtualDom::new_with_props(app, Rc::clone(&mock));
        dom.rebuild_in_place();
        // Drive renders and tasks until nothing is left to run.
        while tokio::time::timeout(Duration::from_millis(200), dom.wait_for_work())
            .await
            .is_ok()
        {
            dom.render_immediate(&mut dioxus::core::NoOpMutations);
        }

        // The write reached the backend, so its continuation (reconcile and
        // write-back) ran in the same poll, after the popover had closed.
        let feed = mock.notifications().await.expect("delayed mock reads");
        assert!(
            feed.iter().all(|n| !n.unread),
            "mark-all-read never completed, so the test proved nothing"
        );
    }
}
