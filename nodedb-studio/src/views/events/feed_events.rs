//! Recent events: the events of the selected feed and Studio's tail.
//!
//! Start tail, Stop tail, and Pause display act on this window's observation
//! session only. The feed delivers to its consumers either way.

use std::rc::Rc;
use std::time::Duration;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::clipboard::use_copier;
use crate::components::icon::{Ico, Icon};
use crate::models::feeds::{ChangeFeed, FeedEvent, export_jsonl};
use crate::services::backend::Backend;
use crate::state::events::EventsUi;
use crate::state::events::feeds::Notice;
use crate::views::events::common::StatusText;
use crate::views::events::feed_tasks;

/// How often a running tail reads.
const POLL: Duration = Duration::from_millis(1500);

/// The events on screen: what the tail shows, then what the feed retained.
pub fn displayed(tail_shown: &[FeedEvent], retained: &[FeedEvent]) -> Vec<FeedEvent> {
    let mut out: Vec<FeedEvent> = tail_shown.to_vec();
    for e in retained {
        if !out.iter().any(|x| x.id == e.id) {
            out.push(e.clone());
        }
    }
    out
}

/// What the status line says about the tail.
pub fn status_label(running: bool, stopped_with_events: bool, paused: bool, held: usize) -> String {
    match (running, paused) {
        (true, false) => "Receiving…".to_string(),
        (true, true) => format!("Display paused · {held} new"),
        (false, _) if stopped_with_events => "Tail stopped".to_string(),
        (false, _) => "Not tailing".to_string(),
    }
}

#[component]
pub fn EventsPanel(feeds: Vec<ChangeFeed>, selected: Option<ChangeFeed>) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut ui = use_context::<EventsUi>().feeds;
    let copier = use_copier();
    let mut export_open = use_signal(|| false);
    let epoch = use_memo(move || ui.read().epoch);
    let selected_id = selected.as_ref().map(|f| f.id.clone());

    // The retained events of the selected feed.
    let mut retained = {
        let backend = backend.clone();
        let id = selected_id.clone();
        use_resource(use_reactive!(|id| {
            let backend = backend.clone();
            let (_conn, _epoch) = (conn(), epoch());
            async move {
                match id {
                    Some(id) => backend.recent_events(&id).await,
                    None => Ok(Vec::new()),
                }
            }
        }))
    };

    // A running tail reads on a timer. The resource restarts when the session
    // changes, which drops the old loop.
    let session = use_memo(move || ui.read().tail.session.clone());
    {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let session = session();
            async move {
                let Some(session) = session else { return };
                loop {
                    tokio::time::sleep(POLL).await;
                    match backend.read_feed_events(&session, 20).await {
                        Ok(events) => {
                            if !events.is_empty() {
                                // Commit only what this read delivered.
                                let _ = backend.commit_feed_offsets(&session).await;
                            }
                            let mut w = ui.write();
                            w.tail.error = None;
                            w.tail.receive(events);
                        }
                        Err(e) => ui.write().tail.error = Some(e.to_string()),
                    }
                }
            }
        });
    }

    let state = resource_state(&retained);
    let retained_events: Vec<FeedEvent> = state.loaded().cloned().unwrap_or_default();
    let (tail, chosen_event) = {
        let u = ui.read();
        (u.tail.clone(), u.selected_event.clone())
    };
    let own_tail = selected.as_ref().is_some_and(|f| tail.is_for(&f.id));
    let shown_by_tail: &[FeedEvent] = if own_tail { &tail.shown } else { &[] };
    let events = displayed(shown_by_tail, &retained_events);
    let selected_event = chosen_event
        .filter(|id| events.iter().any(|e| &e.id == id))
        .or_else(|| events.first().map(|e| e.id.clone()));
    let running = own_tail && tail.running();
    let label = status_label(
        running,
        own_tail && !tail.shown.is_empty(),
        own_tail && tail.paused,
        tail.held.len(),
    );
    let feed_name = selected
        .as_ref()
        .map(|f| f.name.clone())
        .unwrap_or_default();
    let export_text = export_jsonl(&events);
    let export_count = events.len();

    rsx! {
        div { class: "toolbar nowrap",
            label { class: "toolbar-select",
                span { class: "sr-only", "Feed" }
                select {
                    class: "select native",
                    "aria-label": "Feed",
                    onchange: {
                        let backend = backend.clone();
                        move |e| {
                            let (backend, id) = (backend.clone(), e.value());
                            spawn_forever(async move { feed_tasks::select(ui, backend, id).await });
                        }
                    },
                    for f in feeds.iter() {
                        option {
                            key: "{f.id}",
                            value: "{f.id}",
                            selected: selected_id.as_deref() == Some(f.id.as_str()),
                            "{f.name}"
                        }
                    }
                }
            }
            span { class: "status tail-status", role: "status",
                span { class: if running && !tail.paused { "status-dot ok" } else if running { "status-dot warn" } else { "status-dot idle" } }
                "{label}"
            }
            div { class: "toolbar-spacer" }
            if running {
                button {
                    class: "btn",
                    title: "Close Studio's session. {feed_name} keeps delivering.",
                    onclick: {
                        let backend = backend.clone();
                        move |_| {
                            let backend = backend.clone();
                            spawn_forever(async move { feed_tasks::stop_tail(ui, backend).await });
                        }
                    },
                    Ico { icon: Icon::Square, size: 16 }
                    "Stop tail"
                }
            } else {
                button {
                    class: "btn",
                    disabled: selected.is_none(),
                    title: "Open a Studio session on {feed_name}, starting at the latest event",
                    onclick: {
                        let backend = backend.clone();
                        let id = selected_id.clone();
                        move |_| {
                            let Some(id) = id.clone() else { return };
                            let backend = backend.clone();
                            spawn_forever(async move { feed_tasks::start_tail(ui, backend, id).await });
                        }
                    },
                    Ico { icon: Icon::Play, size: 16 }
                    "Start tail"
                }
            }
            button {
                class: "btn",
                disabled: !running,
                title: if running { "Freeze this table. The session keeps reading and the feed keeps delivering." } else { "Start the tail to pause its display" },
                onclick: move |_| {
                    let mut w = ui.write();
                    if w.tail.paused {
                        w.tail.resume();
                    } else {
                        w.tail.paused = true;
                    }
                },
                if tail.paused {
                    Ico { icon: Icon::Play, size: 16 }
                    "Resume display"
                } else {
                    Ico { icon: Icon::Pause, size: 16 }
                    "Pause display"
                }
            }
            div { class: "anchor",
                button {
                    class: "btn",
                    "aria-haspopup": "menu",
                    "aria-expanded": "{export_open()}",
                    onclick: move |e| { e.stop_propagation(); export_open.set(!export_open()); },
                    Ico { icon: Icon::Download, size: 16 }
                    "Export"
                }
                if export_open() {
                    div {
                        class: "popover align-right",
                        role: "menu",
                        onclick: move |e| e.stop_propagation(),
                        onkeydown: move |e| { if e.key() == Key::Escape { export_open.set(false); } },
                        button {
                            class: "menu-item",
                            role: "menuitem",
                            disabled: export_count == 0,
                            onclick: {
                                let text = export_text.clone();
                                move |_| {
                                    copier.copy(text.clone());
                                    export_open.set(false);
                                    ui.write().notice = Some(Notice::ok(format!(
                                        "Copied {export_count} displayed events as JSON Lines."
                                    )));
                                }
                            },
                            Ico { icon: Icon::Copy, size: 16 }
                            "Copy displayed events ({export_count})"
                        }
                        button {
                            class: "menu-item",
                            role: "menuitem",
                            disabled: true,
                            title: "Exporting a retained range of the feed is not designed yet",
                            Ico { icon: Icon::History, size: 16 }
                            "Export a retained range…"
                        }
                        button {
                            class: "menu-item",
                            role: "menuitem",
                            disabled: true,
                            title: "Export jobs need a server transport that is not designed yet",
                            Ico { icon: Icon::Upload, size: 16 }
                            "Start a new export job…"
                        }
                    }
                }
            }
        }
        if let Some(e) = tail.error.clone() {
            div { class: "callout warn notice-line", role: "alert",
                Ico { icon: Icon::TriangleAlert, size: 16 }
                span { class: "notice-text", "The tail could not read: {e} The events below are from before the error." }
            }
        }
        AsyncView {
            loading: state.is_loading(),
            empty: state.is_empty() && shown_by_tail.is_empty(),
            error: state.error_message(),
            retriable: state.is_retriable(),
            on_retry: move |_| retained.restart(),
            empty_message: "This feed has retained no events.",
        }
        if !events.is_empty() {
            div { class: "dt-wrap",
                table { class: "dt",
                    thead {
                        tr {
                            th { "Received (UTC)" }
                            th { "Operation" }
                            th { "Record" }
                            th { "Delivery" }
                        }
                    }
                    tbody {
                        for e in events.iter() {
                            {
                                let id = e.id.clone();
                                let is_selected = selected_event.as_deref() == Some(e.id.as_str());
                                rsx! {
                                    tr {
                                        key: "{e.id}",
                                        class: if is_selected { "clickable selected" } else { "clickable" },
                                        "aria-selected": "{is_selected}",
                                        tabindex: "0",
                                        onclick: {
                                            let id = id.clone();
                                            move |_| {
                                                let mut w = ui.write();
                                                w.selected_event = Some(id.clone());
                                                w.attempts_for_event = false;
                                            }
                                        },
                                        onkeydown: move |k| {
                                            if k.key() == Key::Enter {
                                                ui.write().selected_event = Some(id.clone());
                                            }
                                        },
                                        td { class: "mono", "{e.received_at}" }
                                        td { "{e.op.label()}" }
                                        td { class: "mono", "{e.record_id}" }
                                        td { StatusText { tone: e.delivery.tone(), text: e.delivery.label().to_string() } }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        div { class: "panel-footer note-footer",
            Ico { icon: Icon::Info, size: 16 }
            span {
                "Display pause does not stop delivery."
                if tail.skipped > 0 {
                    " {tail.skipped} events were not held while the display was paused."
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::MockConnectionService;
    use crate::services::feed_data::FeedData;

    #[tokio::test]
    async fn tail_events_sit_above_the_retained_ones_without_repeats() {
        let svc = MockConnectionService::ready();
        let retained = svc.recent_events("feed_01").await.expect("events");
        let session = svc.open_feed_session("feed_01").await.expect("session");
        let live = svc.read_feed_events(&session, 2).await.expect("read");
        let all = displayed(&live, &retained);
        assert_eq!(all.len(), 5);
        assert_eq!(all[0].id, live[0].id);
        // An event in both lists shows once.
        assert_eq!(displayed(&retained, &retained).len(), 3);
    }

    #[test]
    fn the_status_names_every_tail_state() {
        assert_eq!(status_label(true, true, false, 0), "Receiving…");
        assert_eq!(status_label(true, true, true, 4), "Display paused · 4 new");
        assert_eq!(status_label(false, true, false, 0), "Tail stopped");
        assert_eq!(status_label(false, false, false, 0), "Not tailing");
    }
}
