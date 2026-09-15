//! The disconnected state: the entry screen. Pick a saved connection (or add
//! one) to enter the studio. Preferences is reachable here via the topbar link.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::services::backend::Backend;
use crate::state::connection::{ActiveConnection, ConnectError, apply_connect};
use crate::state::connections_registry::{ConnStatus, Credentials, SavedConnection};
use crate::state::ui::ModalKind;

#[component]
pub fn ConnectionManager() -> Element {
    let registry = use_context::<Signal<Vec<SavedConnection>>>();
    let mut active = use_context::<Signal<Option<ActiveConnection>>>();
    let mut modal = use_context::<Signal<Option<ModalKind>>>();
    let service = use_context::<Rc<dyn Backend>>();
    let mut connect_error = use_context::<Signal<ConnectError>>();

    rsx! {
        div { class: "conn-manager",
            div { class: "cm-topbar",
                div { class: "cm-brand-mini",
                    div { class: "logo", "N" }
                    div { "NodeDB " span { "Studio" } }
                }
                div { class: "cm-topbar-actions",
                    a { onclick: move |_| modal.set(Some(ModalKind::Preferences)), "Preferences" }
                    a { "Docs" }
                    span { class: "version", "dev" }
                }
            }

            div { class: "cm-content",
                div { class: "cm-hero",
                    h1 { "Connect to a database" }
                    p { "Pick a saved connection or add a new one. Workspace, tools, and admin open after you connect." }
                }

                div { class: "cm-section-head",
                    h2 { "Saved connections" }
                    button { class: "btn", onclick: move |_| modal.set(Some(ModalKind::NewConnection)), "+ New connection" }
                }

                div { class: "cm-grid",
                    for conn in registry.read().iter().cloned() {
                        ConnectionCard {
                            key: "{conn.name}",
                            conn: conn.clone(),
                            on_connect: {
                                let service = service.clone();
                                // The stored profile IS this card's explicit
                                // username. A profile-less entry yields a blank,
                                // which the seam rejects with MissingUsername and
                                // the app root renders, rather than defaulting to
                                // `admin`. New connections collect a username in
                                // the modal (see modals/new_connection.rs).
                                let creds = Credentials {
                                    username: conn
                                        .profile
                                        .as_ref()
                                        .map(|p| p.user.clone())
                                        .unwrap_or_default(),
                                    password: None,
                                };
                                move |name: String| {
                                    // Async at the seam: clone the Rc into the task and
                                    // write the signals (Copy) only after the await
                                    // resolves. Clearing the error first makes a stale
                                    // failure disappear the moment a new attempt starts.
                                    let service = service.clone();
                                    let creds = creds.clone();
                                    connect_error.set(ConnectError(None));
                                    // spawn_forever so the task survives this
                                    // screen being swapped for the studio shell.
                                    spawn_forever(async move {
                                        let result = service.connect(&name, &creds).await;
                                        let err = apply_connect(&mut active.write(), result);
                                        connect_error.set(ConnectError(err));
                                    });
                                }
                            },
                        }
                    }
                    button { class: "cm-card cm-new-card", onclick: move |_| modal.set(Some(ModalKind::NewConnection)),
                        div { class: "plus", "+" }
                        div { "New connection" }
                    }
                }

                div { class: "cm-section-head", h2 { "Recent activity" } }
                div { class: "cm-recent",
                    RecentItem { fail: false, what_connected: "local-nodedb-dev", detail: " · 142ms last query", when: "2m ago" }
                    RecentItem { fail: true, what_connected: "test-nodedb", detail: " · password expired", when: "1h ago", verb: "Auth failed on" }
                    RecentItem { fail: false, what_connected: "prod-replica-eu", detail: " · added TLS cert", when: "yesterday", verb: "Edited" }
                    RecentItem { fail: false, what_connected: "staging-cluster", detail: " · ran 14 queries", when: "yesterday" }
                }
            }
        }
    }
}

#[component]
fn ConnectionCard(conn: SavedConnection, on_connect: EventHandler<String>) -> Element {
    let connectable = conn.status.is_connectable();
    let name = conn.name.clone();
    let (pill_class, pill_text, dot_style) = match conn.status {
        ConnStatus::Online => ("pill ok", "online", ""),
        ConnStatus::ReadOnly => ("pill warn", "read-only", ""),
        ConnStatus::Offline => ("pill", "offline", "background:var(--text-tertiary)"),
    };
    let dbs = conn
        .db_count
        .map(|n| n.to_string())
        .unwrap_or_else(|| "—".to_string());
    let ping = conn.ping.clone().unwrap_or_else(|| "—".to_string());

    rsx! {
        button {
            class: "cm-card",
            disabled: !connectable,
            onclick: move |_| {
                if connectable {
                    on_connect.call(name.clone());
                }
            },
            div { class: "cm-card-top",
                div { class: "cm-card-name", "{conn.name}" }
                span { class: "{pill_class}", span { class: "dot", style: "{dot_style}" } "{pill_text}" }
            }
            div { class: "cm-card-meta", "{conn.meta}" }
            div { class: "cm-card-stats",
                div { class: "stat", span { class: "v", "{dbs}" } span { class: "l", "DBs" } }
                div { class: "stat", span { class: "v", "{ping}" } span { class: "l", "ping" } }
                div { class: "stat", span { class: "v", "{conn.server}" } span { class: "l", "server" } }
            }
        }
    }
}

#[component]
fn RecentItem(
    fail: bool,
    what_connected: String,
    detail: String,
    when: String,
    #[props(default = "Connected to".to_string())] verb: String,
) -> Element {
    rsx! {
        div { class: "cm-recent-item",
            span { class: if fail { "dot fail" } else { "dot" } }
            span { class: "what", "{verb} " strong { "{what_connected}" } "{detail}" }
            span { class: "when", "{when}" }
        }
    }
}
