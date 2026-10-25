//! The Console page: the session on the left, the command history on the
//! right. The transcript and prompt live above the router, so a visit to
//! another page keeps them.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::resource_state;
use crate::components::clipboard::use_copier;
use crate::components::code_editor::CodeEditor;
use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side, region_style};
use crate::models::console::{HistoryQuery, HistoryWindow};
use crate::services::backend::Backend;
use crate::state::connection::ActiveConnection;
use crate::state::console::ConsoleState;
use crate::state::query::TxEpoch;
use crate::state::workspace::{Surface, Workspace};
use crate::views::console::history::HistoryBody;
use crate::views::console::tasks::{self, Env};
use crate::views::console::toolbar::ConsoleToolbar;
use crate::views::console::transcript::{Line, PROMPT, Transcript, lines};

pub const SURFACE: Surface = Surface::Console;
pub const HISTORY_SIZE: PanelSize = PanelSize::width(316.0, 260.0, 520.0);
pub const PROMPT_SIZE: PanelSize = PanelSize::height(150.0, 100.0, 420.0);

/// The transcript as plain text, for the clipboard.
fn plain(entries: &[crate::state::console::ConsoleEntry]) -> String {
    lines(entries)
        .into_iter()
        .map(|l| match l {
            Line::Command(_, t) => format!("{PROMPT}{t}"),
            Line::Continued(_, t) => format!("     \u{2026}> {t}"),
            Line::Table(t) | Line::Note(t) | Line::Error(t) => t,
            Line::Blank => String::new(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[component]
pub fn ConsolePage() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut state = use_context::<Signal<ConsoleState>>();
    let tx_epoch = use_context::<TxEpoch>();
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let layout = use_context::<Signal<Workspace>>();
    let env = Env {
        state,
        tx: tx_epoch,
    };
    let epoch = use_memo(move || state.read().epoch);
    let filter = use_memo(move || {
        let s = state.read();
        HistoryQuery {
            protocol: s.filter,
            window: s.window,
        }
    });
    let protocol = use_memo(move || state.read().protocol);

    let mut history = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            let (query, _epoch, _conn) = (filter(), epoch(), conn());
            async move { backend.console_history(&query).await }
        }
    });
    // The prompt's own recall list: every command of the current protocol.
    let recall = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            let (p, _epoch, _conn) = (protocol(), epoch(), conn());
            async move {
                backend
                    .console_history(&HistoryQuery {
                        protocol: Some(p),
                        window: HistoryWindow::AllTime,
                    })
                    .await
            }
        }
    });
    let transaction = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            let (_conn, _epoch) = (conn(), *tx_epoch.0.read());
            async move { backend.transaction().await }
        }
    });
    let databases = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            let _conn = conn();
            async move { backend.databases().await }
        }
    });

    // Keep the newest line in view.
    let end = use_signal(|| None::<Rc<MountedData>>);
    let _scroll = use_resource(move || async move {
        let _count = state.read().entries.len();
        if let Some(el) = end.read().clone() {
            let _ = el.scroll_to(ScrollBehavior::Instant).await;
        }
    });

    let history_state = resource_state(&history);
    let tx = transaction.read().clone().and_then(Result::ok);
    let names = databases
        .read()
        .clone()
        .and_then(Result::ok)
        .unwrap_or_default();
    let recalled = recall
        .read()
        .clone()
        .and_then(Result::ok)
        .unwrap_or_default();
    let session_db = active
        .read()
        .as_ref()
        .map(|c| c.current_database.clone())
        .unwrap_or_default();
    let copier = use_copier();

    let (entries, prompt, proto, busy) = {
        let s = state.read();
        (s.entries.clone(), s.prompt.clone(), s.protocol, s.busy)
    };
    let prompt_style = region_style(&layout.read(), SURFACE, "prompt", PROMPT_SIZE);
    let transcript_text = plain(&entries);
    let has_entries = !entries.is_empty();
    let mut menu = use_signal(|| false);
    let mut recent = use_signal(|| false);

    let run = {
        let backend = backend.clone();
        let db = session_db.clone();
        move || tasks::start_prompt(env, backend.clone(), &db)
    };

    rsx! {
        div { class: "page-body",
            div { class: "workbench console-page",
                Panel { surface: SURFACE, id: "console", title: "Console", class: "fill console-panel",
                    ConsoleToolbar { databases: names, tx }
                    div { class: "console-terminal",
                        div { class: "console-tools",
                            button {
                                class: "btn sm",
                                disabled: !has_entries,
                                onclick: move |_| state.write().clear(),
                                Ico { icon: Icon::Trash, size: 16 }
                                "Clear"
                            }
                            div { class: "anchor",
                                button {
                                    class: "btn sm",
                                    disabled: !has_entries,
                                    "aria-haspopup": "menu",
                                    "aria-expanded": "{menu()}",
                                    onclick: move |e| { e.stop_propagation(); menu.set(!menu()); },
                                    Ico { icon: Icon::Download, size: 16 }
                                    "Export transcript"
                                }
                                if menu() {
                                    div {
                                        class: "popover align-right",
                                        role: "menu",
                                        "aria-label": "Export transcript",
                                        onclick: move |e| e.stop_propagation(),
                                        button {
                                            class: "menu-item",
                                            role: "menuitem",
                                            onclick: {
                                                let text = transcript_text.clone();
                                                move |_| { copier.copy(text.clone()); menu.set(false); }
                                            },
                                            Ico { icon: Icon::Copy, size: 16 }
                                            "Copy transcript"
                                        }
                                        div { class: "menu-sep" }
                                        button {
                                            class: "menu-item",
                                            role: "menuitem",
                                            disabled: true,
                                            title: "Saving a file needs the native file dialog, which Studio does not ship yet",
                                            Ico { icon: Icon::Download, size: 16 }
                                            "Save to file"
                                        }
                                    }
                                }
                            }
                        }
                        Transcript { entries, protocol: proto, busy, end }
                    }
                    ResizeHandle { surface: SURFACE, target: "prompt", size: PROMPT_SIZE, side: Side::After }
                    div { class: "console-prompt", style: "{prompt_style}",
                        CodeEditor {
                            class: "prompt-code",
                            value: prompt,
                            protocol: proto,
                            label: "{proto.label()} prompt",
                            placeholder: proto.placeholder(),
                            on_input: move |text: String| state.write().edit_prompt(text),
                            on_run: { let run = run.clone(); move |_| run() },
                            on_key: {
                                let recalled = recalled.clone();
                                move |e: KeyboardEvent| {
                                    if e.modifiers().ctrl() {
                                        match e.key() {
                                            Key::ArrowUp => { e.prevent_default(); state.write().recall(&recalled, true); }
                                            Key::ArrowDown => { e.prevent_default(); state.write().recall(&recalled, false); }
                                            _ => {}
                                        }
                                    }
                                }
                            },
                        }
                        div { class: "prompt-bar",
                            div { class: "anchor",
                                button {
                                    class: "btn ghost sm",
                                    "aria-haspopup": "menu",
                                    "aria-expanded": "{recent()}",
                                    onclick: move |e| { e.stop_propagation(); recent.set(!recent()); },
                                    Ico { icon: Icon::Clock, size: 16 }
                                    "History ({proto.label()})"
                                    Ico { icon: Icon::ChevronUp, size: 16 }
                                }
                                if recent() {
                                    div {
                                        class: "popover align-left up recent-popover",
                                        role: "menu",
                                        "aria-label": "Recent {proto.label()} commands",
                                        onclick: move |e| e.stop_propagation(),
                                        if recalled.is_empty() {
                                            div { class: "async-empty", "No earlier commands." }
                                        }
                                        for h in recalled.iter().take(8).cloned() {
                                            button {
                                                key: "{h.id}",
                                                class: "menu-item recent-item",
                                                role: "menuitem",
                                                onclick: {
                                                    let text = h.text.clone();
                                                    move |_| { state.write().edit_prompt(text.clone()); recent.set(false); }
                                                },
                                                span { class: "mono truncate", "{h.text}" }
                                            }
                                        }
                                        div { class: "menu-sep" }
                                        div { class: "menu-hint", "Ctrl+Up and Ctrl+Down step through them." }
                                    }
                                }
                            }
                            span { class: "toolbar-spacer" }
                            button {
                                class: "btn primary",
                                disabled: busy,
                                title: "Run (Ctrl+Enter)",
                                onclick: { let run = run.clone(); move |_| run() },
                                Ico { icon: Icon::Play, size: 16 }
                                "Run"
                                kbd { class: "kbd-hint", "Ctrl" Ico { icon: Icon::CornerDownLeft, size: 12 } }
                            }
                        }
                    }
                }
                ResizeHandle { surface: SURFACE, target: "history", size: HISTORY_SIZE, side: Side::After }
                Panel { surface: SURFACE, id: "history", title: "Command history", size: HISTORY_SIZE,
                    HistoryBody { history: history_state, on_retry: move |_| history.restart() }
                }
            }
        }
    }
}
