//! The Connection Manager: saved connections, the selected connection's
//! details, and the connection log, in a full window with no studio shell.
//!
//! It shows while no session is open, and on demand while one is (Manage
//! connections), when the studio stays mounted behind it. The selection
//! lives in a root signal so the dialog can pick a connection it just saved.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::connector::use_connector;
use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::components::view_menu::ViewMenuFor;
use crate::models::shell::APP_VERSION;
use crate::services::backend::Backend;
use crate::state::connection::{ActiveConnection, ConnectError};
use crate::state::connection_form::ConnectionForm;
use crate::state::connections_registry::SavedConnection;
use crate::state::preferences::Preferences;
use crate::state::ui::{ManagerOpen, ManagerSelection, ModalKind, Popover, PrefsPane};
use crate::state::workspace::Surface;
use crate::views::connections::activity::ActivityTable;
use crate::views::connections::details::ConnectionDetails;
use crate::views::connections::saved::SavedTable;
use crate::views::connections::sort::{Column, Sort, clicked, sorted};

const SURFACE: Surface = Surface::Connections;
const ACTIVITY_SIZE: PanelSize = PanelSize::height(300.0, 160.0, 520.0);
const DETAILS_SIZE: PanelSize = PanelSize::width(400.0, 300.0, 560.0);

/// The highlighted connection: the chosen one while it is listed, else the
/// first.
fn effective<'a>(list: &'a [SavedConnection], chosen: Option<&str>) -> Option<&'a SavedConnection> {
    chosen
        .and_then(|n| list.iter().find(|c| c.name == n))
        .or_else(|| list.first())
}

#[component]
pub fn ConnectionManager() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let registry = use_context::<Signal<Vec<SavedConnection>>>();
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let prefs = use_context::<Signal<Preferences>>();
    let mut selection = use_context::<Signal<ManagerSelection>>();
    let mut manager = use_context::<Signal<ManagerOpen>>();
    let mut modal = use_context::<Signal<Option<ModalKind>>>();
    let mut form = use_context::<Signal<ConnectionForm>>();
    let mut error = use_context::<Signal<ConnectError>>();
    let mut popover = use_context::<Signal<Option<Popover>>>();
    let connector = use_connector();
    let mut search = use_signal(String::new);
    let mut sort = use_signal(|| None::<Sort>);

    // Reading the registry signal re-runs this when a save or a duplicate
    // changes it, so the table never shows a list older than the registry.
    let mut connections = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            let _registry = registry.read().len();
            async move { backend.list_connections().await }
        }
    });
    let mut events = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            async move { backend.connection_activity().await }
        }
    });
    let state = resource_state(&connections);
    let event_state = resource_state(&events);
    let list = state.loaded().cloned().unwrap_or_default();
    let visible: Vec<SavedConnection> = sorted(
        &list
            .iter()
            .filter(|c| c.matches(&search.read()))
            .cloned()
            .collect::<Vec<_>>(),
        *sort.read(),
    );
    let chosen = selection.read().0.clone();
    let highlighted = effective(&list, chosen.as_deref()).cloned();
    let connected = active.read().as_ref().map(|c| c.name.clone());
    let offset = prefs.read().utc_offset_minutes;
    let open_popover = *popover.read();

    // Write handlers outlive the screen: a successful connect swaps this
    // screen for the studio, and a scope-bound task would die at its await.
    let duplicate = {
        let backend = backend.clone();
        move |name: String| {
            let backend = backend.clone();
            spawn_forever(async move {
                let mut registry = registry;
                match backend.duplicate_connection(&name).await {
                    Ok(copy) => {
                        if let Ok(list) = backend.list_connections().await {
                            registry.set(list);
                        }
                        selection.set(ManagerSelection(Some(copy.name)));
                    }
                    Err(e) => error.set(ConnectError(Some(e))),
                }
            });
        }
    };

    rsx! {
        div { class: "manager", onclick: move |_| popover.set(None),
            header { class: "manager-header",
                div { class: "manager-brand",
                    span { class: "brand-mark", Ico { icon: Icon::Database, size: 22 } }
                    span { class: "brand-name", "NodeDB Studio" }
                }
                div { class: "header-actions",
                    if let Some(name) = connected.clone() {
                        button {
                            class: "btn",
                            onclick: move |_| manager.set(ManagerOpen(false)),
                            Ico { icon: Icon::ChevronLeft, size: 16 }
                            "Back to {name}"
                        }
                    }
                    div { class: "header-icons",
                        button {
                            class: "icon-btn header-icon",
                            title: "Preferences",
                            "aria-label": "Preferences",
                            onclick: move |e| {
                                e.stop_propagation();
                                popover.set(None);
                                modal.set(Some(ModalKind::Preferences(PrefsPane::Appearance)));
                            },
                            Ico { icon: Icon::Settings, size: 20 }
                        }
                        div { class: "anchor",
                            button {
                                class: if open_popover == Some(Popover::View) { "icon-btn header-icon pressed" } else { "icon-btn header-icon" },
                                title: "View",
                                "aria-label": "View",
                                "aria-haspopup": "menu",
                                "aria-expanded": "{open_popover == Some(Popover::View)}",
                                onclick: move |e| {
                                    e.stop_propagation();
                                    let now = *popover.peek();
                                    popover.set(if now == Some(Popover::View) { None } else { Some(Popover::View) });
                                },
                                Ico { icon: Icon::Columns2, size: 20 }
                            }
                            if open_popover == Some(Popover::View) {
                                ViewMenuFor { surface: SURFACE, navigation: false }
                            }
                        }
                    }
                }
            }
            main { class: "manager-main",
                h1 { class: "page-title", "Connections" }
                div { class: "page-body",
                    div { class: "workbench",
                        div { class: "workbench col",
                            Panel {
                                surface: SURFACE,
                                id: "saved",
                                title: "Saved connections",
                                class: "fill",
                                div { class: "toolbar nowrap",
                                    div { class: "search-field grow",
                                        Ico { icon: Icon::Search, size: 16 }
                                        input {
                                            class: "input",
                                            placeholder: "Search connections…",
                                            "aria-label": "Search connections",
                                            value: "{search}",
                                            oninput: move |e| search.set(e.value()),
                                        }
                                    }
                                    div { class: "toolbar-spacer" }
                                    button {
                                        class: "btn primary",
                                        onclick: move |_| {
                                            form.set(ConnectionForm::blank());
                                            modal.set(Some(ModalKind::NewConnection));
                                        },
                                        Ico { icon: Icon::Plus, size: 16 }
                                        "Connection"
                                    }
                                }
                                AsyncView {
                                    loading: state.is_loading(),
                                    empty: state.is_empty(),
                                    error: state.error_message(),
                                    retriable: state.is_retriable(),
                                    on_retry: move |_| connections.restart(),
                                    empty_message: "No saved connections yet. Choose + Connection to add one.",
                                }
                                if state.loaded().is_some() {
                                    SavedTable {
                                        rows: visible.clone(),
                                        selected: highlighted.as_ref().map(|c| c.name.clone()),
                                        connected: connected.clone(),
                                        sort: *sort.read(),
                                        offset,
                                        search: search.read().clone(),
                                        on_sort: move |col: Column| {
                                            let next = clicked(*sort.peek(), col);
                                            sort.set(Some(next));
                                        },
                                        on_select: move |name: String| selection.set(ManagerSelection(Some(name))),
                                        on_open: {
                                            let connector = connector.clone();
                                            let list = list.clone();
                                            move |name: String| {
                                                if let Some(c) = list.iter().find(|c| c.name == name) {
                                                    selection.set(ManagerSelection(Some(name.clone())));
                                                    connector.start(name, c.credentials());
                                                }
                                            }
                                        },
                                    }
                                    div { class: "dt-footer",
                                        "{list.len()} saved connections"
                                    }
                                }
                            }
                            ResizeHandle { surface: SURFACE, target: "activity", size: ACTIVITY_SIZE, side: Side::After }
                            Panel {
                                surface: SURFACE,
                                id: "activity",
                                title: "Recent activity",
                                size: ACTIVITY_SIZE,
                                AsyncView {
                                    loading: event_state.is_loading(),
                                    empty: event_state.is_empty(),
                                    error: event_state.error_message(),
                                    retriable: event_state.is_retriable(),
                                    on_retry: move |_| events.restart(),
                                    empty_message: "No connection activity yet.",
                                }
                                if let Some(rows) = event_state.loaded() {
                                    ActivityTable { events: rows.clone(), offset }
                                }
                            }
                        }
                        ResizeHandle { surface: SURFACE, target: "details", size: DETAILS_SIZE, side: Side::After }
                        Panel {
                            surface: SURFACE,
                            id: "details",
                            title: "Connection details",
                            size: DETAILS_SIZE,
                            if let Some(c) = highlighted.clone() {
                                {
                                    let name = c.name.clone();
                                    let creds = c.credentials();
                                    let is_connected = connected.as_deref() == Some(c.name.as_str());
                                    let connector = connector.clone();
                                    let duplicate = duplicate.clone();
                                    let edit = c.clone();
                                    rsx! {
                                        ConnectionDetails {
                                            conn: c,
                                            connected: is_connected,
                                            offset,
                                            on_connect: {
                                                let name = name.clone();
                                                move |_| connector.start(name.clone(), creds.clone())
                                            },
                                            on_open_workspace: move |_| manager.set(ManagerOpen(false)),
                                            on_edit: move |_| {
                                                form.set(ConnectionForm::from_saved(&edit));
                                                modal.set(Some(ModalKind::NewConnection));
                                            },
                                            on_duplicate: move |_| duplicate(name.clone()),
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            footer { class: "manager-footer",
                if let Some(name) = connected {
                    span { class: "footer-item",
                        span { class: "status-dot ok", "aria-hidden": "true" }
                        "Connected to {name}"
                    }
                } else {
                    span { class: "footer-item",
                        span { class: "status-dot idle", "aria-hidden": "true" }
                        "Disconnected"
                    }
                }
                span { class: "footer-spacer" }
                span { class: "footer-item", "{prefs.read().zone_label()} · NodeDB Studio {APP_VERSION}" }
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
    async fn the_first_entry_is_highlighted_until_one_is_chosen() {
        let list = registry().await;
        assert_eq!(
            effective(&list, None).map(|c| c.name.as_str()),
            Some("Local development")
        );
        assert_eq!(
            effective(&list, Some("Production")).map(|c| c.name.as_str()),
            Some("Production")
        );
    }

    /// A chosen connection that left the list falls back, never shows stale.
    #[tokio::test]
    async fn a_removed_choice_falls_back_to_the_first() {
        let list = registry().await;
        assert_eq!(
            effective(&list, Some("ghost")).map(|c| c.name.as_str()),
            Some("Local development")
        );
        assert!(effective(&[], Some("x")).is_none());
    }
}
