//! The notification drawer: a right-hand panel opened from the header bell.
//!
//! Single source of truth: the bell badge and this drawer read the one shared
//! `Signal<AsyncState<Vec<Notification>>>` store, seeded once at the seam in
//! `app.rs`. The drawer filters it by the connection's capabilities and by
//! its Unread / All tab, then renders `Loading`, `Empty`, and `Error`
//! through the shared `AsyncView`. A write (mark read, mark all read) changes
//! the store only after the seam accepts it, so the badge and the list
//! cannot disagree with the server.
//!
//! Every entry is typed and scoped: its kind picks the icon, its scope names
//! the object, and its action link opens the page that explains it. The
//! drawer shows server events only. It has no conflict-resolution buttons.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::AsyncView;
use crate::components::icon::{Ico, Icon};
use crate::models::format::stamp;
use crate::models::notification::{Notification, NotificationTarget, ScopeKind, Severity};
use crate::routes::Route;
use crate::routes::tabs::EventsTab;
use crate::services::async_state::AsyncState;
use crate::services::backend::Backend;
use crate::services::error::StudioError;
use crate::state::connection::{ActiveConnection, Capabilities, ConnectError};
use crate::state::notifications::{
    Filter, apply_mark_all_read, apply_mark_read, listed, unread_count, visible,
};
use crate::state::preferences::Preferences;
use crate::state::sync::SyncUi;
use crate::state::ui::Popover;

fn severity_icon(s: Severity) -> (Icon, &'static str) {
    match s {
        Severity::Success => (Icon::CircleCheck, "ok"),
        Severity::Info => (Icon::Info, "info"),
        Severity::Warn => (Icon::TriangleAlert, "warn"),
        Severity::Err => (Icon::CircleAlert, "err"),
    }
}

fn scope_icon(k: ScopeKind) -> Icon {
    match k {
        ScopeKind::Collection | ScopeKind::Database => Icon::Database,
        ScopeKind::Role => Icon::User,
        ScopeKind::Job => Icon::CalendarClock,
        ScopeKind::Cluster => Icon::Network,
    }
}

/// The reference day for "today": the newest entry's. Entries from that day
/// show a time, older ones a date and a time.
fn newest(items: &[Notification]) -> i64 {
    items.iter().map(|n| n.at).max().unwrap_or(0)
}

#[component]
pub fn NotificationDrawer() -> Element {
    // The one shared store and the reload handle behind the error Retry.
    let mut store = use_context::<Signal<AsyncState<Vec<Notification>>>>();
    let mut reload = use_context::<Resource<()>>();
    let mut popover = use_context::<Signal<Option<Popover>>>();
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let prefs = use_context::<Signal<Preferences>>();
    let mut error = use_context::<Signal<ConnectError>>();
    let backend = use_context::<Rc<dyn Backend>>();
    let sync_rejects = use_context::<SyncUi>().rejects;
    let nav = use_navigator();
    let mut tab = use_signal(Filter::default);

    // Writes reconcile into the store only after the seam answers. A failed
    // write goes to the app-wide error bar: the list stays as the server
    // still holds it. spawn_forever, because the drawer can close mid-write
    // and the root store still has to reconcile.
    let mark_all = {
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            spawn_forever(async move {
                let result = backend.mark_all_read().await;
                let failure = apply_mark_all_read(&mut store.write(), result);
                if failure.is_some() {
                    error.set(ConnectError(failure));
                }
            });
        }
    };
    let mark_one = {
        let backend = backend.clone();
        move |id: String| {
            let backend = backend.clone();
            spawn_forever(async move {
                let result: Result<(), StudioError> = backend.mark_read(&id).await;
                let failure = apply_mark_read(&mut store.write(), &id, result);
                if failure.is_some() {
                    error.set(ConnectError(failure));
                }
            });
        }
    };

    let caps: Capabilities = match active.read().as_ref() {
        Some(c) => c.capabilities,
        None => return rsx! {},
    };
    // The filtered view, owned: `Empty` re-derives from what THIS connection
    // and tab show. The read guard drops at the end of the statement.
    let all: AsyncState<Vec<Notification>> = store
        .read()
        .project(|raw| visible(raw, &caps).cloned().collect());
    let unread = store
        .read()
        .loaded()
        .map_or(0, |items| unread_count(items, &caps));
    let filter = *tab.read();
    let rows: Vec<Notification> = store
        .read()
        .loaded()
        .map(|items| listed(items, &caps, filter))
        .unwrap_or_default();
    let reference = all.loaded().map_or(0, |items| newest(items));
    let offset = prefs.read().utc_offset_minutes;

    let empty_message = match filter {
        Filter::Unread => "You are all caught up.",
        Filter::All => "No notifications for this connection.",
    };

    rsx! {
        aside {
            class: "drawer",
            "aria-label": "Notifications",
            onclick: move |e| e.stop_propagation(),
            header { class: "drawer-header",
                h2 { class: "drawer-title", "Notifications" }
                button {
                    class: "icon-btn",
                    title: "Close",
                    "aria-label": "Close notifications",
                    onclick: move |_| popover.set(None),
                    Ico { icon: Icon::X, size: 18 }
                }
            }
            div { class: "drawer-tabs",
                div { class: "tab-bar sub", role: "tablist",
                    button {
                        class: if filter == Filter::Unread { "tab active" } else { "tab" },
                        role: "tab",
                        "aria-selected": "{filter == Filter::Unread}",
                        onclick: move |_| tab.set(Filter::Unread),
                        "Unread ({unread})"
                    }
                    button {
                        class: if filter == Filter::All { "tab active" } else { "tab" },
                        role: "tab",
                        "aria-selected": "{filter == Filter::All}",
                        onclick: move |_| tab.set(Filter::All),
                        "All"
                    }
                }
                button {
                    class: "link-btn",
                    disabled: unread == 0,
                    onclick: {
                        let mark_all = mark_all.clone();
                        move |_| mark_all()
                    },
                    "Mark all read"
                }
            }
            div { class: "drawer-body",
                AsyncView {
                    loading: all.is_loading(),
                    empty: all.is_empty() || (all.loaded().is_some() && rows.is_empty()),
                    error: all.error_message(),
                    retriable: all.is_retriable(),
                    empty_message: empty_message.to_string(),
                    on_retry: move |_| reload.restart(),
                }
                for n in rows {
                    {
                        let id = n.id.clone();
                        let read_id = n.id.clone();
                        let mark_one = mark_one.clone();
                        let mark_for_link = mark_one.clone();
                        let route = n.target.route();
                        let (icon, tone) = severity_icon(n.severity);
                        let when = stamp(n.at, reference, offset);
                        rsx! {
                            article {
                                key: "{id}",
                                class: if n.unread { "notice unread" } else { "notice" },
                                span { class: "notice-icon {tone}", Ico { icon, size: 22 } }
                                div { class: "notice-main",
                                    div { class: "notice-top",
                                        span { class: "notice-title", "{n.title}" }
                                        span { class: "notice-time", "{when}" }
                                        if n.unread {
                                            button {
                                                class: "unread-dot",
                                                title: "Mark as read",
                                                "aria-label": "Mark {n.title} as read",
                                                onclick: move |_| mark_one(read_id.clone()),
                                            }
                                        } else {
                                            span { class: "read-dot", "aria-hidden": "true" }
                                        }
                                    }
                                    p { class: "notice-body", "{n.body}" }
                                    div { class: "notice-bottom",
                                        span { class: "notice-scope",
                                            Ico { icon: scope_icon(n.scope_kind), size: 16 }
                                            "{n.scope}"
                                        }
                                        button {
                                            class: "link-btn",
                                            onclick: {
                                                let id = n.id.clone();
                                                let route = route.clone();
                                                let target = (n.target, n.subject.clone());
                                                move |_| {
                                                    mark_for_link(id.clone());
                                                    popover.set(None);
                                                    // A rejection opens on its own write.
                                                    if let (NotificationTarget::SyncRejectedWrites, Some(m)) = &target {
                                                        let mut rejects = sync_rejects;
                                                        rejects.write().selected = Some(m.clone());
                                                    }
                                                    nav.push(route.clone());
                                                }
                                            },
                                            "{n.target.action_label()}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            footer { class: "drawer-footer",
                button {
                    class: "link-btn",
                    onclick: move |_| {
                        popover.set(None);
                        nav.push(Route::Events { tab: EventsTab::Notifications });
                    },
                    Ico { icon: Icon::FileText, size: 16 }
                    "View history"
                }
                span { class: "footer-sep", "aria-hidden": "true" }
                button {
                    class: "link-btn",
                    disabled: true,
                    title: "Notification preferences are not designed yet",
                    Ico { icon: Icon::Settings, size: 16 }
                    "Notification preferences"
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::MockConnectionService;
    use crate::services::connection_service::ConnectionService;

    #[tokio::test]
    async fn the_reference_day_is_the_newest_entry() {
        let feed = MockConnectionService::ready()
            .notifications()
            .await
            .expect("mock feed");
        assert_eq!(newest(&feed), feed[0].at);
        assert_eq!(newest(&[]), 0);
    }

    #[test]
    fn every_kind_of_scope_has_an_icon() {
        for k in [
            ScopeKind::Collection,
            ScopeKind::Database,
            ScopeKind::Role,
            ScopeKind::Job,
            ScopeKind::Cluster,
        ] {
            let _ = scope_icon(k);
        }
    }

    /// Success and error entries must read differently by shape and tone.
    #[test]
    fn severities_have_distinct_tones() {
        let tones: std::collections::HashSet<_> = [
            Severity::Success,
            Severity::Info,
            Severity::Warn,
            Severity::Err,
        ]
        .iter()
        .map(|s| severity_icon(*s).1)
        .collect();
        assert_eq!(tones.len(), 4);
    }
}
