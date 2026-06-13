//! The disconnected state: pick a saved connection to enter the studio.
//!
//! Phase 1 renders a minimal, functional list to prove the connect flow drives
//! the top-level state machine. The faithful card grid, status pills, and
//! new-connection modal are ported in later phases.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::services::connection_service::ConnectionService;
use crate::state::connection::ActiveConnection;
use crate::state::connections_registry::SavedConnection;

#[component]
pub fn ConnectionManager() -> Element {
    let registry = use_context::<Signal<Vec<SavedConnection>>>();
    let mut active = use_context::<Signal<Option<ActiveConnection>>>();
    let service = use_context::<Rc<dyn ConnectionService>>();

    rsx! {
        div { class: "cm",
            h1 { "Connect to a database" }
            p { "Pick a saved connection. Workspace, tools, and admin open after you connect." }
            div { class: "cm-grid",
                for conn in registry.read().iter().cloned() {
                    ConnectionCard {
                        key: "{conn.name}",
                        conn: conn.clone(),
                        on_connect: {
                            let service = service.clone();
                            move |name: String| {
                                if let Some(session) = service.connect(&name) {
                                    active.set(Some(session));
                                }
                            }
                        },
                    }
                }
            }
        }
    }
}

#[component]
fn ConnectionCard(conn: SavedConnection, on_connect: EventHandler<String>) -> Element {
    let connectable = conn.status.is_connectable();
    let name = conn.name.clone();
    rsx! {
        button {
            class: "cm-card",
            disabled: !connectable,
            onclick: move |_| {
                if connectable {
                    on_connect.call(name.clone());
                }
            },
            div { class: "cm-card-name", "{conn.name}" }
            div { class: "cm-card-meta", "{conn.meta}" }
        }
    }
}
