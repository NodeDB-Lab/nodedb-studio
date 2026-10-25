//! The Console's single toolbar row: protocol, target database, session,
//! and the transaction.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::components::tx_badge::TxBadge;
use crate::models::console::Protocol;
use crate::models::workbench::TxState;
use crate::state::connection::ActiveConnection;
use crate::state::console::ConsoleState;

#[component]
pub fn ConsoleToolbar(databases: Vec<String>, tx: Option<TxState>) -> Element {
    let mut state = use_context::<Signal<ConsoleState>>();
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let mut session = use_signal(|| false);
    let (protocol, database) = {
        let s = state.read();
        (s.protocol, s.database.clone())
    };
    let conn = active.read().clone();
    let session_db = conn
        .as_ref()
        .map(|c| c.current_database.clone())
        .unwrap_or_default();
    let target = database.clone().unwrap_or_else(|| session_db.clone());

    rsx! {
        div { class: "toolbar nowrap console-toolbar",
            span { class: "toolbar-label", "Protocol" }
            div { class: "seg accent", role: "radiogroup", "aria-label": "Protocol",
                for p in Protocol::ALL {
                    button {
                        key: "{p.label()}",
                        class: if p == protocol { "active" } else { "" },
                        role: "radio",
                        "aria-checked": "{p == protocol}",
                        onclick: move |_| state.write().set_protocol(p),
                        "{p.label()}"
                    }
                }
            }
            span { class: "toolbar-label", "Target" }
            label { class: "toolbar-select",
                Ico { icon: Icon::Database, size: 16 }
                select {
                    class: "select native compact",
                    "aria-label": "Target database",
                    onchange: move |e| state.write().database = Some(e.value()),
                    for d in databases.iter() {
                        option { key: "{d}", value: "{d}", selected: *d == target, "{d}" }
                    }
                    if !databases.contains(&target) {
                        option { value: "{target}", selected: true, "{target}" }
                    }
                }
            }
            span { class: "toolbar-label", "Session" }
            div { class: "anchor",
                button {
                    class: "toolbar-select session-chip",
                    "aria-haspopup": "dialog",
                    "aria-expanded": "{session()}",
                    onclick: move |e| { e.stop_propagation(); session.set(!session()); },
                    span { class: "status-dot ok" }
                    "Connected"
                    Ico { icon: Icon::ChevronDown, size: 16 }
                }
                if session() {
                    if let Some(c) = conn {
                        div {
                            class: "popover align-left session-popover",
                            role: "dialog",
                            "aria-label": "Session",
                            onclick: move |e| e.stop_propagation(),
                            onkeydown: move |e| { if e.key() == Key::Escape { session.set(false); } },
                            div { class: "menu-title", "Session" }
                            div { class: "kv kv-compact",
                                div { class: "kv-row", span { "Connection" } span { "{c.name}" } }
                                div { class: "kv-row", span { "User" } span { "{c.user}" } }
                                div { class: "kv-row", span { "Role" } span { "{c.role}" } }
                                div { class: "kv-row", span { "Database" } span { "{session_db}" } }
                            }
                        }
                    }
                }
            }
            span { class: "toolbar-label", "Transaction" }
            TxBadge { state: tx.unwrap_or_default() }
        }
    }
}
