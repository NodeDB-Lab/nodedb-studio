//! The editor toolbar: Run, Explain, the database, the saved-query actions,
//! and the transaction. One row.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::components::tx_badge::TxBadge;
use crate::models::saved_query::{QueryLibrary, SaveQuery};
use crate::models::workbench::{TxAccess, TxState};
use crate::services::async_state::AsyncState;
use crate::services::backend::Backend;
use crate::state::connection::ActiveConnection;
use crate::state::query::{QueryTab, QueryWorkspace, TxEpoch};
use crate::views::query::tasks::{self, Env, TxOp};

/// The save request for `tab`, a new version of the query it edits.
fn save_request(
    tab: &QueryTab,
    library: &QueryLibrary,
    id: Option<String>,
    folder: Option<String>,
) -> SaveQuery {
    let folder = match &id {
        Some(existing) => library.get(existing).and_then(|q| q.folder.clone()),
        None => folder,
    };
    SaveQuery {
        id,
        name: tab.title.trim().to_string(),
        folder,
        database: tab.database.clone(),
        sql: tab.sql.clone(),
        params: tab.params.clone(),
        tags: tab.tags.clone(),
    }
}

#[component]
pub fn Toolbar(
    databases: Vec<String>,
    tx: Option<TxState>,
    library: AsyncState<QueryLibrary>,
) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let mut ws = use_context::<Signal<QueryWorkspace>>();
    let tx_epoch = use_context::<TxEpoch>();
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let env = Env::new(ws, tx_epoch);
    let mut run_menu = use_signal(|| false);
    let mut save_as = use_signal(|| None::<String>);
    let mut folder = use_signal(String::new);

    let active_db = active
        .read()
        .as_ref()
        .map(|c| c.current_database.clone())
        .unwrap_or_default();
    let w = ws.read();
    let tab = w.active().cloned();
    let selected = w.library_selected.clone();
    drop(w);
    let lib = library.loaded().cloned().unwrap_or_default();
    let tx_state = tx.unwrap_or_default();

    let reason = tab.as_ref().and_then(|t| tasks::blocked(t, &active_db));
    let no_tab = tab.is_none();
    let can_run = !no_tab && reason.is_none();
    let run_title = match (&tab, &reason) {
        (None, _) => "Open a query tab first".to_string(),
        (_, Some(why)) => why.clone(),
        (_, None) => "Run (Ctrl+Enter)".to_string(),
    };
    let mode = tab.as_ref().map(|t| t.mode).unwrap_or_default();
    let (saved_id, dirty) = tab
        .as_ref()
        .map(|t| (t.saved.clone(), t.dirty()))
        .unwrap_or_default();
    let can_save = !no_tab && (saved_id.is_none() || dirty);
    let save_title = if no_tab {
        "Open a query tab first"
    } else if saved_id.is_some() && !dirty {
        "No unsaved changes"
    } else if saved_id.is_some() {
        "Save a new version"
    } else {
        "Save as a named query"
    };
    let open_target = selected.as_ref().and_then(|id| lib.get(id).cloned());

    let tab_title = tab.as_ref().map(|t| t.title.clone()).unwrap_or_default();

    let begin = {
        let backend = backend.clone();
        move |access: TxAccess| {
            run_menu.set(false);
            tasks::start_transaction(env, backend.clone(), TxOp::Begin(access));
        }
    };

    rsx! {
        div { class: "toolbar nowrap query-toolbar",
            div { class: "anchor",
                div { class: "btn-group",
                    button {
                        class: "btn primary",
                        disabled: !can_run,
                        title: "{run_title}",
                        onclick: {
                            let backend = backend.clone();
                            let db = active_db.clone();
                            move |_| tasks::start_run(env, backend.clone(), &db, 0)
                        },
                        Ico { icon: Icon::Play, size: 16 }
                        "Run"
                    }
                    button {
                        class: "btn primary",
                        title: "More ways to run",
                        "aria-label": "More ways to run",
                        "aria-haspopup": "menu",
                        "aria-expanded": "{run_menu()}",
                        disabled: no_tab,
                        onclick: move |e| { e.stop_propagation(); run_menu.set(!run_menu()); },
                        Ico { icon: Icon::ChevronDown, size: 16 }
                    }
                }
                if run_menu() {
                    div {
                        class: "popover align-left",
                        role: "menu",
                        "aria-label": "Run",
                        onclick: move |e| e.stop_propagation(),
                        div { class: "menu-title", "Transaction" }
                        button {
                            class: "menu-item",
                            role: "menuitem",
                            disabled: tx_state.is_open(),
                            title: if tx_state.is_open() { "A transaction is already open" } else { "Begin a transaction that cannot write" },
                            onclick: { let mut begin = begin.clone(); move |_| begin(TxAccess::ReadOnly) },
                            Ico { icon: Icon::Eye, size: 16 }
                            "Begin read-only transaction"
                        }
                        button {
                            class: "menu-item",
                            role: "menuitem",
                            disabled: tx_state.is_open(),
                            title: if tx_state.is_open() { "A transaction is already open" } else { "Begin a transaction that can write" },
                            onclick: { let mut begin = begin.clone(); move |_| begin(TxAccess::ReadWrite) },
                            Ico { icon: Icon::Pencil, size: 16 }
                            "Begin read-write transaction"
                        }
                    }
                }
            }
            button {
                class: "btn",
                disabled: !can_run,
                title: if can_run { mode.note().to_string() } else { run_title.clone() },
                onclick: {
                    let backend = backend.clone();
                    let db = active_db.clone();
                    move |_| tasks::start_explain(env, backend.clone(), &db)
                },
                Ico { icon: Icon::Workflow, size: 16 }
                "{mode.label()}"
            }
            if !databases.is_empty() {
                label { class: "toolbar-select",
                    Ico { icon: Icon::Database, size: 16 }
                    select {
                        class: "select native compact",
                        "aria-label": "Database",
                        disabled: no_tab,
                        title: "The database this statement runs against",
                        onchange: move |e| {
                            if let Some(t) = ws.write().active_mut() { t.database = e.value(); }
                        },
                        for d in databases.iter() {
                            option {
                                key: "{d}",
                                value: "{d}",
                                selected: tab.as_ref().is_some_and(|t| t.database == *d),
                                "{d}"
                            }
                        }
                    }
                }
            }
            span { class: "toolbar-spacer" }
            button {
                class: "btn",
                disabled: !can_save,
                title: "{save_title}",
                onclick: {
                    let backend = backend.clone();
                    let tab = tab.clone();
                    let lib = lib.clone();
                    move |_| {
                        let Some(t) = &tab else { return };
                        match &t.saved {
                            Some(id) => {
                                let req = save_request(t, &lib, Some(id.clone()), None);
                                let (backend, tab_id) = (backend.clone(), t.id);
                                spawn_forever(async move { tasks::save(env, backend, tab_id, req).await });
                            }
                            None => save_as.set(Some(t.title.clone())),
                        }
                    }
                },
                Ico { icon: Icon::Save, size: 16 }
                span { class: "tb-label", "Save" }
            }
            div { class: "anchor",
                button {
                    class: "btn",
                    disabled: no_tab,
                    title: "Save as a new named query",
                    "aria-label": "Save as",
                    "aria-haspopup": "dialog",
                    "aria-expanded": "{save_as().is_some()}",
                    onclick: {
                        let name = tab_title.clone();
                        move |e| {
                            e.stop_propagation();
                            if save_as().is_some() { save_as.set(None); } else { save_as.set(Some(name.clone())); }
                        }
                    },
                    span { class: "tb-label", "Save as" }
                    span { class: "tb-short", "As\u{2026}" }
                }
                if let Some(name) = save_as() {
                    div {
                        class: "popover align-right save-as-popover",
                        role: "dialog",
                        "aria-label": "Save as",
                        onclick: move |e| e.stop_propagation(),
                        onkeydown: move |e| { if e.key() == Key::Escape { save_as.set(None); } },
                        div { class: "field",
                            label { r#for: "save-as-name", "Name" }
                            input {
                                id: "save-as-name",
                                class: "input",
                                value: "{name}",
                                oninput: move |e| save_as.set(Some(e.value())),
                            }
                        }
                        if !lib.folders.is_empty() {
                            div { class: "field",
                                label { r#for: "save-as-folder", "Folder" }
                                select {
                                    id: "save-as-folder",
                                    class: "select native",
                                    onchange: move |e| folder.set(e.value()),
                                    option { value: "", selected: folder.read().is_empty(), "My queries" }
                                    for f in lib.folders.iter() {
                                        option { key: "{f}", value: "{f}", selected: *folder.read() == *f, "{f}" }
                                    }
                                }
                            }
                        }
                        div { class: "popover-actions",
                            button { class: "btn sm", onclick: move |_| save_as.set(None), "Cancel" }
                            button {
                                class: "btn sm primary",
                                disabled: name.trim().is_empty(),
                                onclick: {
                                    let backend = backend.clone();
                                    let tab = tab.clone();
                                    let lib = lib.clone();
                                    let name = name.clone();
                                    move |_| {
                                        let Some(t) = &tab else { return };
                                        let mut named = t.clone();
                                        named.title = name.clone();
                                        let f = folder.peek().clone();
                                        let req = save_request(&named, &lib, None, (!f.is_empty()).then_some(f));
                                        let (backend, tab_id) = (backend.clone(), t.id);
                                        save_as.set(None);
                                        spawn_forever(async move { tasks::save(env, backend, tab_id, req).await });
                                    }
                                },
                                "Save"
                            }
                        }
                    }
                }
            }
            button {
                class: "btn",
                disabled: open_target.is_none(),
                title: if open_target.is_some() { "Open the selected saved query" } else { "Select a saved query in the library first" },
                onclick: move |_| {
                    if let Some(q) = &open_target { ws.write().open_saved(q); }
                },
                Ico { icon: Icon::FolderOpen, size: 16 }
                span { class: "tb-label", "Open" }
            }
            if tx_state.is_open() {
                span { class: "toolbar-sep" }
                span { class: "muted", "Transaction" }
                TxBadge { state: tx_state }
                button {
                    class: "btn",
                    disabled: tx_state == TxState::Failed,
                    title: if tx_state == TxState::Failed { "A failed transaction cannot commit: roll it back" } else { "Commit the transaction" },
                    onclick: {
                        let backend = backend.clone();
                        move |_| tasks::start_transaction(env, backend.clone(), TxOp::Commit)
                    },
                    "Commit"
                }
                button {
                    class: "btn",
                    title: "Roll the transaction back",
                    onclick: {
                        let backend = backend.clone();
                        move |_| tasks::start_transaction(env, backend.clone(), TxOp::Rollback)
                    },
                    "Rollback"
                }
            }
        }
    }
}
