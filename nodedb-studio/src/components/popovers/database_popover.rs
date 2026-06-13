//! Database picker popover: filterable list of the active connection's
//! databases; selecting one updates the current database.

use dioxus::prelude::*;

use crate::state::connection::ActiveConnection;
use crate::state::ui::Popover;

#[component]
pub fn DatabasePopover() -> Element {
    let mut active = use_context::<Signal<Option<ActiveConnection>>>();
    let mut popover = use_context::<Signal<Option<Popover>>>();
    let mut filter = use_signal(String::new);

    let conn = active.read();
    let Some(c) = conn.as_ref() else {
        return rsx! {};
    };
    let current = c.current_database.clone();
    let q = filter.read().to_lowercase();
    let dbs: Vec<String> = c
        .databases
        .iter()
        .filter(|d| d.to_lowercase().contains(&q))
        .cloned()
        .collect();

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
                for db in dbs {
                    {
                        let name = db.clone();
                        let item_class = if db == current { "db-popover-item current" } else { "db-popover-item" };
                        rsx! {
                            div {
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
            }
        }
    }
}
