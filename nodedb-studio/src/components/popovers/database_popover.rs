//! The database selector: the active connection's databases, searchable,
//! with the current one checked. Opens below the header's database chip.
//!
//! A pick changes the catalog context, never the connection. The list is
//! read through the seam when the popover opens (it mounts only while open),
//! so a database created since connect shows up. A pick writes through the
//! seam first and moves the header only when the server accepts it.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::services::async_state::AsyncState;
use crate::services::backend::Backend;
use crate::state::connection::{ActiveConnection, ConnectError, apply_database_switch};
use crate::state::ui::Popover;

/// `databases` whose name contains `filter`, case-insensitively, in order.
fn matching(databases: &[String], filter: &str) -> Vec<String> {
    let q = filter.trim().to_lowercase();
    databases
        .iter()
        .filter(|d| d.to_lowercase().contains(&q))
        .cloned()
        .collect()
}

/// `12 databases`, or `3 of 12 databases` while a search narrows the list.
fn count_label(shown: usize, total: usize) -> String {
    let noun = if total == 1 { "database" } else { "databases" };
    if shown == total {
        format!("{total} {noun}")
    } else {
        format!("{shown} of {total} {noun}")
    }
}

#[component]
pub fn DatabasePopover() -> Element {
    let mut active = use_context::<Signal<Option<ActiveConnection>>>();
    let mut popover = use_context::<Signal<Option<Popover>>>();
    let mut error = use_context::<Signal<ConnectError>>();
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut filter = use_signal(String::new);
    let mut listed = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            // A switch while the popover is open (from the palette) re-reads
            // the list, so a pick never names a database the new server lacks.
            let _conn = conn();
            async move { backend.databases().await }
        }
    });

    let (name, current, read_only) = match active.read().as_ref() {
        Some(c) => (
            c.name.clone(),
            c.current_database.clone(),
            c.capabilities.readonly,
        ),
        None => return rsx! {},
    };
    let state = resource_state(&listed);
    let total = state.loaded().map_or(0, Vec::len);
    let shown = state
        .loaded()
        .map(|dbs| matching(dbs, &filter.read()))
        .unwrap_or_default();

    rsx! {
        div {
            class: "popover db-selector",
            role: "menu",
            "aria-label": "Databases",
            onclick: move |e| e.stop_propagation(),
            div { class: "menu-scope",
                div { class: "scope-line", "Current connection: " strong { "{name}" } }
                div { class: "scope-sub", "Switch database (changes catalog context)" }
            }
            div { class: "search-field menu-search",
                Ico { icon: Icon::Search, size: 16 }
                input {
                    class: "input",
                    placeholder: "Search databases…",
                    "aria-label": "Search databases",
                    value: "{filter}",
                    oninput: move |e| filter.set(e.value()),
                }
            }
            div { class: "menu-list db-list",
                for db in shown.iter().cloned() {
                    {
                        let is_current = db == current;
                        let pick = db.clone();
                        let backend = backend.clone();
                        rsx! {
                            button {
                                key: "{db}",
                                class: if is_current { "menu-row compact selected" } else { "menu-row compact" },
                                role: "menuitemradio",
                                "aria-checked": "{is_current}",
                                onclick: move |_| {
                                    popover.set(None);
                                    if is_current {
                                        return;
                                    }
                                    let backend = backend.clone();
                                    let pick = pick.clone();
                                    // spawn_forever: the popover closes on the line above.
                                    spawn_forever(async move {
                                        let result = backend.select_database(&pick).await;
                                        let failure = apply_database_switch(&mut active.write(), &pick, result);
                                        if failure.is_some() {
                                            error.set(ConnectError(failure));
                                        }
                                    });
                                },
                                Ico { icon: Icon::Database, size: 18 }
                                span { class: "row-text",
                                    span { class: "row-name", "{db}" }
                                }
                                if is_current {
                                    Ico { icon: Icon::Check, size: 18, class: "row-check" }
                                }
                            }
                        }
                    }
                }
                if let AsyncState::Loaded(_) = &state {
                    if shown.is_empty() {
                        div { class: "menu-empty", "No database matches." }
                    }
                } else {
                    AsyncView {
                        loading: state.is_loading(),
                        empty: state.is_empty(),
                        error: state.error_message(),
                        retriable: state.is_retriable(),
                        on_retry: move |_| listed.restart(),
                        empty_message: "No databases.".to_string(),
                    }
                }
            }
            div { class: "menu-foot",
                span { class: "foot-count", "{count_label(shown.len(), total)}" }
                span { class: "footer-spacer" }
                button { class: "btn", r#type: "button", onclick: move |_| listed.restart(),
                    Ico { icon: Icon::RefreshCw, size: 16 }
                    "Refresh"
                }
                button {
                    class: "btn primary",
                    r#type: "button",
                    disabled: true,
                    title: if read_only {
                        "This connection is read-only: creating a database needs write permission"
                    } else {
                        "Creating a database is not designed yet"
                    },
                    "Create database"
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dbs(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn filter_is_case_insensitive_and_keeps_order() {
        let all = dbs(&["analytics", "events_log", "Staging_A"]);
        assert_eq!(matching(&all, "STAG"), dbs(&["Staging_A"]));
        assert_eq!(matching(&all, "a"), dbs(&["analytics", "Staging_A"]));
    }

    #[test]
    fn empty_filter_keeps_everything() {
        let all = dbs(&["analytics", "events_log"]);
        assert_eq!(matching(&all, ""), all);
        assert_eq!(matching(&all, "  "), all);
    }

    #[test]
    fn count_says_how_many_of_the_total() {
        assert_eq!(count_label(12, 12), "12 databases");
        assert_eq!(count_label(1, 1), "1 database");
        assert_eq!(count_label(3, 12), "3 of 12 databases");
    }
}
