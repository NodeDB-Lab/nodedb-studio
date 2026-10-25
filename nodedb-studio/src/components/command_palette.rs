//! The command palette (Cmd+K). Opens only while connected.
//!
//! Rendered inside the router (via `StudioLayout`) so navigation commands
//! can use the navigator. The open flag and the search text are shared
//! signals provided by `App`. The catalog and the search live in
//! `state::palette`, so this file only draws rows and runs the chosen action.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::connector::use_connector;
use crate::components::icon::{Ico, Icon};
use crate::routes::Route;
use crate::services::backend::Backend;
use crate::state::connection::ActiveConnection;
use crate::state::connection_form::ConnectionForm;
use crate::state::connections_registry::SavedConnection;
use crate::state::palette::{
    Command, Group, PaletteAction, PaletteContext, commands, filter, move_selection,
};
use crate::state::preferences::Preferences;
use crate::state::ui::{ManagerOpen, ManagerSelection, ModalKind, PaletteQuery};
use crate::state::workspace::Workspace;

/// Id of the row at `index`, so the list can scroll it into view.
fn row_id(index: usize) -> String {
    format!("palette-row-{index}")
}

#[component]
pub fn CommandPalette() -> Element {
    let mut open = use_context::<Signal<bool>>();
    let query = use_context::<PaletteQuery>().0;
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let modal = use_context::<Signal<Option<ModalKind>>>();
    let form = use_context::<Signal<ConnectionForm>>();
    let manager = use_context::<Signal<ManagerOpen>>();
    let selection = use_context::<Signal<ManagerSelection>>();
    let prefs = use_context::<Signal<Preferences>>();
    let workspace = use_context::<Signal<Workspace>>();
    let registry = use_context::<Signal<Vec<SavedConnection>>>();
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let connector = use_connector();
    let nav = use_navigator();
    let route = use_route::<Route>();
    let mut selected = use_signal(|| 0usize);

    // Recent items belong to the connection: a switch re-reads them.
    let recents = use_resource(move || {
        let backend = backend.clone();
        let _conn = conn();
        async move { backend.recent_items().await }
    });

    // A new search starts at the first row.
    use_effect(move || {
        let _text = query.read();
        selected.set(0);
    });
    // Keep the highlighted row on screen while the arrows move it.
    use_effect(move || {
        let index = selected();
        if open() {
            let _ = document::eval(&format!(
                "document.getElementById('{}')?.scrollIntoView({{ block: 'nearest' }});",
                row_id(index)
            ));
        }
    });

    if !open() {
        return rsx! {};
    }
    let Some(session) = active.read().clone() else {
        return rsx! {};
    };

    let recent_items = recents
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned()
        .unwrap_or_default();
    let saved = registry.read().clone();
    let surface = route.surface();
    let catalog = commands(&PaletteContext {
        caps: &session.capabilities,
        connection: &session.name,
        connections: &saved,
        recents: &recent_items,
        surface,
    });
    let results = filter(&catalog, &query.read());
    let highlighted = selected().min(results.len().saturating_sub(1));

    // One runner for a click and for Enter.
    let run = {
        let current = session.name.clone();
        Rc::new(move |action: PaletteAction| {
            // Local copies of the signals: the runner is `Fn`, shared by every
            // row, and writing needs a mutable handle.
            let (mut open, mut active, mut modal, mut form) = (open, active, modal, form);
            let (mut manager, mut selection, mut prefs) = (manager, selection, prefs);
            let mut workspace = workspace;
            open.set(false);
            match action {
                PaletteAction::Navigate(r) => {
                    nav.push(r);
                }
                PaletteAction::NewConnection => {
                    form.set(ConnectionForm::blank());
                    modal.set(Some(ModalKind::NewConnection));
                }
                PaletteAction::ManageConnections => {
                    selection.set(ManagerSelection(Some(current.clone())));
                    manager.set(ManagerOpen(true));
                }
                PaletteAction::Disconnect => active.set(None),
                PaletteAction::OpenPreferences(pane) => {
                    modal.set(Some(ModalKind::Preferences(pane)));
                }
                PaletteAction::CycleTheme => {
                    let next = prefs.peek().theme.next();
                    prefs.write().theme = next;
                }
                PaletteAction::RestoreView => workspace.write().restore_default(surface),
                PaletteAction::SwitchConnection(name) => {
                    if let Some(c) = saved.iter().find(|c| c.name == name) {
                        connector.start(name, c.credentials());
                    }
                }
            }
        })
    };

    let on_key = {
        let run = run.clone();
        let actions: Vec<PaletteAction> = results.iter().map(|c| c.action.clone()).collect();
        move |e: KeyboardEvent| match e.key() {
            Key::ArrowDown => {
                e.prevent_default();
                selected.set(move_selection(actions.len(), highlighted, 1));
            }
            Key::ArrowUp => {
                e.prevent_default();
                selected.set(move_selection(actions.len(), highlighted, -1));
            }
            Key::Enter => {
                e.prevent_default();
                if let Some(action) = actions.get(highlighted) {
                    run(action.clone());
                }
            }
            _ => {}
        }
    };

    let typed = query.read().clone();
    // The rows under their group headings, each with its index in the flat
    // list, which the arrow keys walk.
    let mut grouped: Vec<(Group, Vec<(usize, &Command)>)> = Vec::new();
    for (i, c) in results.iter().enumerate() {
        match grouped.last_mut() {
            Some((g, rows)) if *g == c.group => rows.push((i, c)),
            Some(_) | None => grouped.push((c.group, vec![(i, c)])),
        }
    }
    rsx! {
        div {
            class: "palette-scrim",
            onclick: move |_| open.set(false),
            div {
                class: "palette",
                role: "dialog",
                "aria-modal": "true",
                "aria-label": "Command palette",
                onclick: move |e| e.stop_propagation(),
                div { class: "palette-head",
                    Ico { icon: Icon::Search, size: 20 }
                    input {
                        class: "palette-input",
                        placeholder: "Search or run command…",
                        "aria-label": "Search or run command",
                        "aria-controls": "palette-list",
                        autofocus: true,
                        value: "{typed}",
                        oninput: {
                            let mut query = query;
                            move |e| query.set(e.value())
                        },
                        onkeydown: on_key,
                    }
                    div { class: "palette-keys", "aria-hidden": "true",
                        span { class: "key-pair",
                            kbd { Ico { icon: Icon::ArrowUp, size: 12 } }
                            kbd { Ico { icon: Icon::ArrowDown, size: 12 } }
                            "to navigate"
                        }
                        span { class: "key-pair", kbd { "Enter" } "to open" }
                        span { class: "key-pair", kbd { "Esc" } "to close" }
                    }
                }
                div { class: "palette-list", id: "palette-list", role: "listbox",
                    if results.is_empty() {
                        div { class: "palette-empty", "No command matches “{typed}”." }
                    }
                    for (group, rows) in grouped {
                        div { key: "{group.label()}", class: "palette-section",
                            div { class: "palette-group", "{group.label()}" }
                            for (i, c) in rows {
                                {
                                    let action = c.action.clone();
                                    let run = run.clone();
                                    let is_selected = i == highlighted;
                                    rsx! {
                                        button {
                                            key: "{c.id}",
                                            id: "{row_id(i)}",
                                            class: if is_selected { "palette-row selected" } else { "palette-row" },
                                            role: "option",
                                            "aria-selected": "{is_selected}",
                                            onmouseenter: move |_| selected.set(i),
                                            onclick: move |_| run(action.clone()),
                                            Ico { icon: c.icon, size: 22 }
                                            span { class: "palette-text",
                                                span { class: "palette-title", "{c.title}" }
                                                span { class: "palette-desc", "{c.description}" }
                                            }
                                            if let Some(keys) = c.shortcut {
                                                kbd { "{keys}" }
                                            } else if is_selected {
                                                kbd { "Return" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                div { class: "palette-context",
                    span { class: "context-label", "Context" }
                    span { "{session.name} / {session.current_database}" }
                }
            }
        }
    }
}
