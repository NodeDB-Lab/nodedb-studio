//! Notifications: LISTEN/NOTIFY.
//!
//! A notification is transient. It reaches only the sessions listening when
//! it is sent, it has no offset, and it is never replayed. A topic is the
//! durable alternative. Clear display and Pause display change this window
//! and never the listening session. Stop listening ends the session.

use std::rc::Rc;
use std::time::Duration;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::resource_state;
use crate::components::icon::{Ico, Icon};
use crate::components::json_editor::{JsonEditor, JsonState};
use crate::components::json_view::JsonView;
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::models::listen::{NotifyChannel, send_issue};
use crate::routes::tabs::EventsTab;
use crate::services::backend::Backend;
use crate::state::events::EventsUi;
use crate::state::workspace::Surface;
use crate::views::events::common::NoticeLine;
use crate::views::events::notify_tasks;

pub const SURFACE: Surface = Surface::Events(EventsTab::Notifications);
const SEND_SIZE: PanelSize = PanelSize::width(380.0, 300.0, 560.0);
/// How often a listening session polls.
const POLL: Duration = Duration::from_millis(2000);

/// `Showing 10 notifications`.
pub fn showing(n: usize) -> String {
    format!("Showing {n} notification{}", if n == 1 { "" } else { "s" })
}

#[component]
pub fn NotifyTab() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut ui = use_context::<EventsUi>().notify;

    let channels = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let _conn = conn();
            async move { backend.notify_channels().await }
        })
    };
    let channel_state = resource_state(&channels);
    let channel_list: Vec<NotifyChannel> = channel_state.loaded().cloned().unwrap_or_default();

    // A listening session polls on a timer. The resource restarts when the
    // session changes, which drops the old loop.
    let session = use_memo(move || ui.read().session.clone());
    {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let session = session();
            async move {
                let Some(session) = session else { return };
                loop {
                    tokio::time::sleep(POLL).await;
                    match backend.poll_notifications(&session).await {
                        Ok(messages) => {
                            let mut w = ui.write();
                            w.error = None;
                            w.receive(messages);
                        }
                        Err(e) => ui.write().error = Some(e.to_string()),
                    }
                }
            }
        });
    }

    let u = ui.read().clone();
    let listening = u.listening();
    let shown = u.shown.clone();
    let highlighted = u
        .selected
        .clone()
        .filter(|id| shown.iter().any(|m| &m.id == id))
        .or_else(|| shown.first().map(|m| m.id.clone()));
    let detail = u
        .selected
        .as_ref()
        .and_then(|id| shown.iter().find(|m| &m.id == id).cloned());
    let send_blocked = send_issue(&u.send_channel, &u.payload);
    let channel_known = |name: &str| channel_list.iter().any(|c| c.name == name);

    rsx! {
        div { class: "page-body",
            div { class: "workbench events-wb",
                Panel {
                    surface: SURFACE,
                    id: "listener",
                    title: "Notification listener",
                    class: "fill",
                    div { class: "toolbar nowrap",
                        label { class: "toolbar-label", r#for: "nl-channel", "Channel" }
                        select {
                            id: "nl-channel",
                            class: "select native channel-select",
                            disabled: listening,
                            title: if listening { "Stop listening to change the channel" } else { "" },
                            onchange: move |e| ui.write().channel = e.value(),
                            for c in channel_list.iter() {
                                option { key: "{c.name}", value: "{c.name}", selected: c.name == u.channel, "{c.name}" }
                            }
                            if !channel_known(&u.channel) {
                                option { value: "{u.channel}", selected: true, "{u.channel}" }
                            }
                        }
                        div { class: "toolbar-spacer" }
                        if listening {
                            button {
                                class: "btn primary",
                                title: "End Studio's listening session on {u.channel}. Other sessions keep listening.",
                                onclick: {
                                    let backend = backend.clone();
                                    move |_| {
                                        let backend = backend.clone();
                                        spawn_forever(async move { notify_tasks::stop(ui, backend).await });
                                    }
                                },
                                Ico { icon: Icon::Square, size: 16 }
                                "Stop listening"
                            }
                        } else {
                            button {
                                class: "btn primary",
                                disabled: channel_list.is_empty(),
                                title: "Start a Studio session listening on {u.channel}",
                                onclick: {
                                    let backend = backend.clone();
                                    move |_| {
                                        let backend = backend.clone();
                                        spawn_forever(async move { notify_tasks::start(ui, backend).await });
                                    }
                                },
                                Ico { icon: Icon::Play, size: 16 }
                                "Start listening"
                            }
                        }
                        button {
                            class: "btn",
                            disabled: !listening,
                            title: if listening { "Freeze this table. The session keeps listening." } else { "Start listening to pause its display" },
                            onclick: move |_| {
                                let mut w = ui.write();
                                if w.paused { w.resume(); } else { w.paused = true; }
                            },
                            if u.paused {
                                Ico { icon: Icon::Play, size: 16 }
                                "Resume display"
                            } else {
                                Ico { icon: Icon::Pause, size: 16 }
                                "Pause display"
                            }
                        }
                    }
                    div { class: "listener-meta",
                        span { class: "delivery-note",
                            "Transient delivery"
                            span { class: "dot-sep", "•" }
                            "No replay"
                        }
                        div { class: "toolbar-spacer" }
                        if let Some(s) = u.session.clone() {
                            span { "Session {s.id}" }
                            span { class: "dot-sep", "•" }
                        }
                        span { "Listener total: Not reported" }
                        span { class: "status",
                            span { class: if listening { "status-dot ok" } else { "status-dot idle" } }
                            if listening { "Listening" } else { "Not listening" }
                        }
                    }
                    if let Some(e) = u.error.clone() {
                        div { class: "callout err notice-line", role: "alert",
                            Ico { icon: Icon::CircleAlert, size: 16 }
                            span { class: "notice-text", "{e}" }
                        }
                    }
                    if shown.is_empty() {
                        div { class: "async-empty",
                            if listening {
                                "Listening on {u.channel}. Notifications sent from now on appear here."
                            } else {
                                "Start listening to see notifications. Nothing sent earlier can be shown: notifications are not kept."
                            }
                        }
                    } else {
                        div { class: "dt-wrap",
                            table { class: "dt",
                                thead {
                                    tr {
                                        th { "Timestamp (UTC)" }
                                        th { "Channel" }
                                        th { "Payload" }
                                    }
                                }
                                tbody {
                                    for m in shown.iter() {
                                        {
                                            let id = m.id.clone();
                                            let is_selected = highlighted.as_deref() == Some(m.id.as_str());
                                            rsx! {
                                                tr {
                                                    key: "{m.id}",
                                                    class: if is_selected { "clickable selected" } else { "clickable" },
                                                    "aria-selected": "{is_selected}",
                                                    tabindex: "0",
                                                    onclick: {
                                                        let id = id.clone();
                                                        move |_| ui.write().selected = Some(id.clone())
                                                    },
                                                    onkeydown: move |e| {
                                                        if e.key() == Key::Enter {
                                                            ui.write().selected = Some(id.clone());
                                                        }
                                                    },
                                                    td { class: "mono", "{m.at}" }
                                                    td { "{m.channel}" }
                                                    td { class: "mono payload-cell", title: "{m.payload}", "{m.payload}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if let Some(m) = detail {
                        div { class: "message-detail",
                            div { class: "message-detail-head",
                                strong { "Notification" }
                                span { class: "muted", "{m.channel} · {m.at} UTC · not kept after this session" }
                            }
                            JsonView { text: m.payload.clone() }
                        }
                    }
                    div { class: "panel-footer listener-footer",
                        button {
                            class: "link-btn",
                            disabled: shown.is_empty() && u.held.is_empty(),
                            title: "Empty this table. The listening session is not affected.",
                            onclick: move |_| ui.write().clear(),
                            Ico { icon: Icon::ListX, size: 16 }
                            "Clear display"
                        }
                        div { class: "toolbar-spacer" }
                        span { class: "muted", "{showing(shown.len())}" }
                        if u.paused {
                            button {
                                class: "link-btn",
                                onclick: move |_| ui.write().resume(),
                                Ico { icon: Icon::Play, size: 16 }
                                "Resume display"
                                if !u.held.is_empty() { " ({u.held.len()} held)" }
                            }
                        } else {
                            span { class: "live-note",
                                Ico { icon: Icon::Play, size: 16 }
                                "Display live"
                            }
                        }
                    }
                }
                ResizeHandle { surface: SURFACE, target: "send", size: SEND_SIZE, side: Side::After }
                Panel {
                    surface: SURFACE,
                    id: "send",
                    title: "Send notification",
                    size: SEND_SIZE,
                    div { class: "send-form",
                        div { class: "field",
                            label { r#for: "ns-channel", "Channel" }
                            select {
                                id: "ns-channel",
                                class: "select native",
                                onchange: move |e| ui.write().send_channel = e.value(),
                                for c in channel_list.iter() {
                                    option { key: "{c.name}", value: "{c.name}", selected: c.name == u.send_channel, "{c.name}" }
                                }
                            }
                        }
                        div { class: "field grow-field",
                            span { class: "field-label", "Payload (JSON)" }
                            div { class: "json-box fill",
                                JsonEditor {
                                    text: u.payload.clone(),
                                    label: "Notification payload",
                                    on_input: move |t: String| {
                                        let mut w = ui.write();
                                        w.payload = t;
                                        w.sent = None;
                                    },
                                }
                            }
                            JsonState { text: u.payload.clone() }
                        }
                        NoticeLine { notice: u.sent.clone(), on_dismiss: move |_| ui.write().sent = None }
                        button {
                            class: "btn primary block send-btn",
                            disabled: send_blocked.is_some() || u.sending,
                            title: send_blocked.clone().unwrap_or_else(|| format!("Send this payload to {}", u.send_channel)),
                            onclick: {
                                let backend = backend.clone();
                                move |_| {
                                    let backend = backend.clone();
                                    spawn_forever(async move { notify_tasks::send(ui, backend).await });
                                }
                            },
                            Ico { icon: Icon::Send, size: 16 }
                            "Send notification"
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
    fn the_count_agrees_with_its_noun() {
        assert_eq!(showing(0), "Showing 0 notifications");
        assert_eq!(showing(1), "Showing 1 notification");
        assert_eq!(showing(10), "Showing 10 notifications");
    }
}
