//! Database picker popover: filterable list of the active connection's
//! databases; selecting one updates the current database.
//!
//! The list is read through the seam's `databases()` each time the popover
//! opens (it mounts only while open), so a database created since connect
//! shows up. Selecting one changes only the chrome's `current_database`:
//! no seam method switches the server session's database yet.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::AsyncView;
use crate::services::async_state::AsyncState;
use crate::services::backend::Backend;
use crate::state::connection::ActiveConnection;
use crate::state::ui::Popover;

/// `databases` whose name contains `filter`, case-insensitively, in order.
fn matching(databases: &[String], filter: &str) -> Vec<String> {
    let q = filter.to_lowercase();
    databases
        .iter()
        .filter(|d| d.to_lowercase().contains(&q))
        .cloned()
        .collect()
}

#[component]
pub fn DatabasePopover() -> Element {
    let mut active = use_context::<Signal<Option<ActiveConnection>>>();
    let mut popover = use_context::<Signal<Option<Popover>>>();
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut filter = use_signal(String::new);
    let mut listed = use_resource(move || {
        let backend = backend.clone();
        // A switch while the popover is open (from the palette) re-reads the
        // list, so a pick never names a database the new server lacks.
        let _conn = conn();
        async move { backend.databases().await }
    });

    let current = active
        .read()
        .as_ref()
        .map(|c| c.current_database.clone())
        .unwrap_or_default();
    let state = AsyncState::from_value(listed.read().clone());

    rsx! {
        div { class: "db-popover open", onclick: move |e| e.stop_propagation(),
            div { class: "db-popover-search",
                input {
                    placeholder: "filter databases…",
                    value: "{filter}",
                    oninput: move |e| filter.set(e.value()),
                }
            }
            div { class: "db-popover-list",
                if let AsyncState::Loaded(dbs) = &state {
                    for db in matching(dbs, &filter.read()) {
                        {
                            let name = db.clone();
                            let item_class = if db == current { "db-popover-item current" } else { "db-popover-item" };
                            rsx! {
                                div {
                                    key: "{name}",
                                    class: "{item_class}",
                                    onclick: move |_| {
                                        if let Some(c) = active.write().as_mut() {
                                            c.current_database = name.clone();
                                        }
                                        popover.set(None);
                                    },
                                    span { class: "ico", "db" }
                                    span { "{db}" }
                                    span { class: "check", "✓" }
                                }
                            }
                        }
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
    }
}
