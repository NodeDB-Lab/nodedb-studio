//! The Command history panel: earlier commands, filtered by protocol and
//! age. A row loads into the prompt, Rerun runs it, Copy copies it.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::AsyncView;
use crate::components::clipboard::use_copier;
use crate::components::icon::{Ico, Icon};
use crate::models::console::{HistoryEntry, HistoryWindow, Protocol};
use crate::models::format::date_second;
use crate::services::async_state::AsyncState;
use crate::services::backend::Backend;
use crate::state::connection::ActiveConnection;
use crate::state::console::ConsoleState;
use crate::state::preferences::Preferences;
use crate::state::query::TxEpoch;
use crate::views::console::tasks::{self, Env};

/// The value of the "all protocols" option.
const ALL: &str = "all";

fn protocol_of(value: &str) -> Option<Protocol> {
    Protocol::ALL.iter().copied().find(|p| p.label() == value)
}

#[component]
pub fn HistoryBody(history: AsyncState<Vec<HistoryEntry>>, on_retry: EventHandler<()>) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let mut state = use_context::<Signal<ConsoleState>>();
    let env = Env {
        state,
        tx: use_context::<TxEpoch>(),
    };
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let prefs = use_context::<Signal<Preferences>>();
    let copier = use_copier();
    let offset = prefs.read().utc_offset_minutes;
    let (filter, window, selected) = {
        let s = state.read();
        (s.filter, s.window, s.selected.clone())
    };
    let session_db = move || {
        active
            .peek()
            .as_ref()
            .map(|c| c.current_database.clone())
            .unwrap_or_default()
    };

    rsx! {
        div { class: "toolbar nowrap history-filters",
            label { class: "toolbar-select",
                span { class: "select-label", "Protocol" }
                select {
                    class: "select native compact",
                    "aria-label": "Protocol",
                    onchange: move |e| state.write().filter = protocol_of(&e.value()),
                    for p in Protocol::ALL {
                        option { key: "{p.label()}", value: "{p.label()}", selected: filter == Some(p), "{p.label()}" }
                    }
                    option { value: "{ALL}", selected: filter.is_none(), "All" }
                }
            }
            label { class: "toolbar-select",
                Ico { icon: Icon::Calendar, size: 16 }
                select {
                    class: "select native compact",
                    "aria-label": "Time window",
                    onchange: move |e| {
                        if let Some(w) = HistoryWindow::ALL.iter().copied().find(|w| w.label() == e.value()) {
                            state.write().window = w;
                        }
                    },
                    for w in HistoryWindow::ALL {
                        option { key: "{w.label()}", value: "{w.label()}", selected: window == w, "{w.label()}" }
                    }
                }
            }
        }
        AsyncView {
            loading: history.is_loading(),
            empty: history.is_empty(),
            error: history.error_message(),
            retriable: history.is_retriable(),
            on_retry: move |_| on_retry.call(()),
            empty_message: "No commands in this range.",
        }
        if let Some(list) = history.loaded() {
            ul { class: "history-list", "aria-label": "Command history",
                for h in list.iter().cloned() {
                    {
                        let is_selected = selected.as_deref() == Some(h.id.as_str());
                        let (id, text, protocol) = (h.id.clone(), h.text.clone(), h.protocol);
                        let (rerun_text, copy_text) = (h.text.clone(), h.text.clone());
                        let backend = backend.clone();
                        rsx! {
                            li {
                                key: "{h.id}",
                                class: if is_selected { "history-item selected" } else { "history-item" },
                                "aria-selected": "{is_selected}",
                                button {
                                    class: "history-main",
                                    title: "Load into the prompt",
                                    onclick: move |_| {
                                        let mut s = state.write();
                                        s.selected = Some(id.clone());
                                        s.set_protocol(protocol);
                                        s.edit_prompt(text.clone());
                                    },
                                    span { class: "history-at num", "{date_second(h.at, offset)}" }
                                    span { class: "history-text mono", "{h.text}" }
                                }
                                button {
                                    class: "icon-btn sm",
                                    title: "Rerun",
                                    "aria-label": "Rerun {h.text}",
                                    onclick: move |_| {
                                        tasks::start_rerun(env, backend.clone(), &session_db(), protocol, rerun_text.clone());
                                    },
                                    Ico { icon: Icon::Play, size: 16 }
                                }
                                button {
                                    class: "icon-btn sm",
                                    title: "Copy",
                                    "aria-label": "Copy {h.text}",
                                    onclick: move |_| copier.copy(copy_text.clone()),
                                    Ico { icon: Icon::Copy, size: 16 }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_protocol_option_round_trips() {
        for p in Protocol::ALL {
            assert_eq!(protocol_of(p.label()), Some(p));
        }
        assert_eq!(protocol_of(ALL), None);
    }
}
