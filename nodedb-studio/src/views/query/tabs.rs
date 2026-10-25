//! The query tab strip. An amber dot marks a tab with unsaved changes.
//! Closing such a tab asks first: the changes would be lost.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::state::connection::ActiveConnection;
use crate::state::query::QueryWorkspace;

#[component]
pub fn TabStrip() -> Element {
    let mut ws = use_context::<Signal<QueryWorkspace>>();
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let mut closing = use_signal(|| None::<u32>);
    let w = ws.read();
    let current = w.active_id();
    let tabs: Vec<(u32, String, bool)> = w
        .tabs()
        .iter()
        .map(|t| (t.id, t.title.clone(), t.dirty()))
        .collect();
    drop(w);
    let database = active
        .read()
        .as_ref()
        .map(|c| c.current_database.clone())
        .unwrap_or_default();

    rsx! {
        div { class: "query-tabs", role: "tablist", "aria-label": "Query tabs",
            for (id, title, dirty) in tabs {
                {
                    let selected = current == Some(id);
                    let label = title.clone();
                    rsx! {
                        div {
                            key: "{id}",
                            class: if selected { "query-tab active" } else { "query-tab" },
                            button {
                                class: "query-tab-label",
                                role: "tab",
                                "aria-selected": "{selected}",
                                onclick: move |_| ws.write().activate(id),
                                span { class: "query-tab-title", "{title}" }
                                if dirty {
                                    span { class: "unsaved-dot", title: "Unsaved changes" }
                                    span { class: "sr-only", " (unsaved)" }
                                }
                            }
                            button {
                                class: "icon-btn sm query-tab-close",
                                "aria-label": "Close {label}",
                                onclick: move |e| {
                                    e.stop_propagation();
                                    if dirty {
                                        closing.set(Some(id));
                                    } else {
                                        ws.write().close(id);
                                    }
                                },
                                Ico { icon: Icon::X, size: 14 }
                            }
                            if closing() == Some(id) {
                                div {
                                    class: "popover align-left discard-popover",
                                    role: "alertdialog",
                                    "aria-label": "Discard changes",
                                    onclick: move |e| e.stop_propagation(),
                                    onkeydown: move |e| { if e.key() == Key::Escape { closing.set(None); } },
                                    p { "Discard the unsaved changes to " strong { "{label}" } "?" }
                                    div { class: "popover-actions",
                                        button { class: "btn sm", onclick: move |_| closing.set(None), "Keep editing" }
                                        button {
                                            class: "btn sm danger",
                                            onclick: move |_| {
                                                ws.write().close(id);
                                                closing.set(None);
                                            },
                                            "Discard"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            button {
                class: "icon-btn sm query-tab-add",
                title: "New query",
                "aria-label": "New query tab",
                onclick: move |_| { ws.write().new_tab(&database); },
                Ico { icon: Icon::Plus, size: 16 }
            }
        }
    }
}
