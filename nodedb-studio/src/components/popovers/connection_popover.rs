//! The connection switcher: search, the saved connections with the current
//! one checked, and the connection actions. Opens below the header's
//! connection chip.

use dioxus::prelude::*;

use crate::components::connector::use_connector;
use crate::components::icon::{Ico, Icon};
use crate::models::connections::{ConnStatus, ConnectionKind};
use crate::state::connection::ActiveConnection;
use crate::state::connection_form::ConnectionForm;
use crate::state::connections_registry::SavedConnection;
use crate::state::ui::{ManagerOpen, ManagerSelection, ModalKind, Popover};

/// The badge at the right of a row: the strongest reason to look twice.
fn badge(c: &SavedConnection) -> Option<&'static str> {
    match (c.status, c.kind()) {
        (ConnStatus::Offline, _) => Some("Offline"),
        (ConnStatus::NotConnected, _) => Some("Not connected"),
        (ConnStatus::Available, ConnectionKind::Lite) => Some("Lite"),
        (ConnStatus::Available, ConnectionKind::Origin) => None,
    }
}

/// The rows to list: the current connection first, then the others in
/// registry order, each filtered by `search`.
fn rows<'a>(
    registry: &'a [SavedConnection],
    current: &str,
    search: &str,
) -> Vec<&'a SavedConnection> {
    let matching = |c: &&SavedConnection| c.matches(search);
    registry
        .iter()
        .filter(|c| c.name == current)
        .chain(registry.iter().filter(|c| c.name != current))
        .filter(matching)
        .collect()
}

#[component]
pub fn ConnectionPopover() -> Element {
    let mut active = use_context::<Signal<Option<ActiveConnection>>>();
    let mut popover = use_context::<Signal<Option<Popover>>>();
    let mut modal = use_context::<Signal<Option<ModalKind>>>();
    let mut form = use_context::<Signal<ConnectionForm>>();
    let mut manager = use_context::<Signal<ManagerOpen>>();
    let mut selection = use_context::<Signal<ManagerSelection>>();
    let registry = use_context::<Signal<Vec<SavedConnection>>>();
    let connector = use_connector();
    let mut search = use_signal(String::new);

    let current = active.read().as_ref().map(|c| c.name.clone());
    let Some(current) = current else {
        return rsx! {};
    };
    let current_saved = registry.read().iter().find(|c| c.name == current).cloned();
    let listed: Vec<SavedConnection> = rows(&registry.read(), &current, &search.read())
        .into_iter()
        .cloned()
        .collect();

    rsx! {
        div {
            class: "popover switcher",
            role: "menu",
            "aria-label": "Connections",
            onclick: move |e| e.stop_propagation(),
            div { class: "search-field menu-search",
                Ico { icon: Icon::Search, size: 16 }
                input {
                    class: "input",
                    placeholder: "Search connections…",
                    "aria-label": "Search connections",
                    value: "{search}",
                    oninput: move |e| search.set(e.value()),
                }
            }
            div { class: "menu-list",
                for c in listed {
                    {
                        let is_current = c.name == current;
                        let name = c.name.clone();
                        let creds = c.credentials();
                        let connector = connector.clone();
                        let dot = if is_current { "ok" } else { c.status.dot() };
                        // The current row shows the live session's line, which
                        // the last connect measured, not the saved one.
                        let sub = match active.read().as_ref() {
                            Some(a) if is_current => a.sub.clone(),
                            Some(_) | None => c.summary(),
                        };
                        rsx! {
                            button {
                                key: "{c.name}",
                                class: if is_current { "menu-row selected" } else { "menu-row" },
                                role: "menuitemradio",
                                "aria-checked": "{is_current}",
                                onclick: move |_| {
                                    popover.set(None);
                                    if !is_current {
                                        connector.start(name.clone(), creds.clone());
                                    }
                                },
                                span { class: "status-dot {dot}" }
                                span { class: "row-text",
                                    span { class: "row-name", "{c.name}" }
                                    span { class: "row-sub", "{sub}" }
                                }
                                if is_current {
                                    Ico { icon: Icon::Check, size: 18, class: "row-check" }
                                } else if let Some(b) = badge(&c) {
                                    span { class: "badge outline", "{b}" }
                                }
                            }
                        }
                    }
                }
                if registry.read().iter().all(|c| !c.matches(&search.read())) {
                    div { class: "menu-empty", "No connection matches." }
                }
            }
            div { class: "menu-sep" }
            button {
                class: "menu-item",
                role: "menuitem",
                onclick: {
                    let current = current.clone();
                    move |_| {
                        popover.set(None);
                        selection.set(ManagerSelection(Some(current.clone())));
                        manager.set(ManagerOpen(true));
                    }
                },
                Ico { icon: Icon::Settings, size: 18 }
                span { "Manage connections" }
            }
            button {
                class: "menu-item",
                role: "menuitem",
                onclick: move |_| {
                    popover.set(None);
                    form.set(ConnectionForm::blank());
                    modal.set(Some(ModalKind::NewConnection));
                },
                Ico { icon: Icon::Plus, size: 18 }
                span { "New connection" }
            }
            button {
                class: "menu-item",
                role: "menuitem",
                disabled: current_saved.is_none(),
                title: if current_saved.is_none() { "This connection is not in the saved list" } else { "" },
                onclick: move |_| {
                    if let Some(saved) = current_saved.as_ref() {
                        popover.set(None);
                        form.set(ConnectionForm::from_saved(saved));
                        modal.set(Some(ModalKind::NewConnection));
                    }
                },
                Ico { icon: Icon::Pencil, size: 18 }
                span { "Edit current connection" }
            }
            div { class: "menu-sep" }
            button {
                class: "menu-item",
                role: "menuitem",
                onclick: move |_| { popover.set(None); active.set(None); },
                Ico { icon: Icon::Unplug, size: 18 }
                span { "Disconnect" }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::MockConnectionService;
    use crate::services::connection_service::ConnectionService;

    /// The saved-connection registry, read through the seam as the app does.
    async fn registry() -> Vec<SavedConnection> {
        MockConnectionService::ready()
            .list_connections()
            .await
            .expect("mock registry")
    }

    #[tokio::test]
    async fn the_current_connection_leads_and_the_rest_keep_registry_order() {
        let saved = registry().await;
        let names: Vec<_> = rows(&saved, "staging-cluster", "")
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(
            names,
            [
                "staging-cluster",
                "Local development",
                "Production",
                "test-cluster"
            ]
        );
    }

    #[tokio::test]
    async fn search_filters_and_keeps_the_current_first() {
        let saved = registry().await;
        let names: Vec<_> = rows(&saved, "staging-cluster", "test")
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(names, ["test-cluster"]);
        let origin: Vec<_> = rows(&saved, "staging-cluster", "origin")
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(origin, ["staging-cluster", "Production", "test-cluster"]);
    }

    /// Offline outranks Not connected, which outranks the Lite tag.
    #[tokio::test]
    async fn badges_follow_the_design() {
        let saved = registry().await;
        let badge_of = |n: &str| saved.iter().find(|c| c.name == n).and_then(badge);
        assert_eq!(badge_of("Local development"), Some("Lite"));
        assert_eq!(badge_of("Production"), Some("Not connected"));
        assert_eq!(badge_of("test-cluster"), Some("Offline"));
        assert_eq!(badge_of("staging-cluster"), None);
    }
}
