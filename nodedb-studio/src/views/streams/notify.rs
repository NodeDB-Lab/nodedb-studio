//! Streams · LISTEN/NOTIFY: channel list + a live pub/sub tail, read through
//! the seam. Split into a fetch wrapper (`StreamsNotify`, owns the two async
//! reads) and a pure presentational component (`NotifyPanes`, takes
//! `AsyncState` as input) so the four states are render-testable.
//!
//! NodeDB has no LISTEN/NOTIFY: no `LISTEN` keyword in `nodedb-sql`, no
//! pub/sub, no client path. The only `NOTIFY` in the server is
//! `CREATE ALERT … NOTIFY TOPIC/WEBHOOK`, which is alert routing. So the seam
//! methods behind this screen stand for a feature that does not exist yet, and
//! the models carry only fields a future implementation could actually fill.
//! The mockup showed a publisher identity per message; nothing in NodeDB can
//! produce one, so the tail shows the channel instead of inventing a field.
//!
//! Which channel is selected is view state, not a seam field: the server has
//! no opinion about what the user clicked. It defaults to the first loaded
//! channel, the same way the Explorer derives its default selection.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::AsyncView;
use crate::models::streams::{NotifyChannel, NotifyMessage};
use crate::services::async_state::AsyncState;
use crate::services::backend::Backend;

/// The default selected channel: the first one the seam returned, in its own
/// order. `None` while loading, on error, or when there are no channels —
/// never a hardcoded name, which rots silently the moment the fixture changes.
pub fn default_channel(channels: &[NotifyChannel]) -> Option<String> {
    channels.first().map(|c| c.name.clone())
}

#[component]
pub fn StreamsNotify() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();

    let mut channels = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            async move { backend.notify_channels().await }
        }
    });
    let mut messages = use_resource(move || {
        let backend = backend.clone();
        async move { backend.notify_messages().await }
    });

    // Explicit user choice; `None` means "whatever the load defaults to".
    let mut picked = use_signal(|| None::<String>);

    // Clone out of the resource guards immediately — never hold one across a
    // render or an await.
    let channels_state = AsyncState::from_value(channels.read().clone());
    let messages_state = AsyncState::from_value(messages.read().clone());

    let selected = picked.read().clone().or_else(|| {
        channels_state
            .loaded()
            .and_then(|list| default_channel(list))
    });

    rsx! {
        NotifyPanes {
            channels: channels_state,
            messages: messages_state,
            selected,
            on_pick: move |name: String| picked.set(Some(name)),
            on_retry_channels: move |_| channels.restart(),
            on_retry_messages: move |_| messages.restart(),
        }
    }
}

#[component]
fn NotifyPanes(
    channels: AsyncState<Vec<NotifyChannel>>,
    messages: AsyncState<Vec<NotifyMessage>>,
    selected: Option<String>,
    on_pick: EventHandler<String>,
    on_retry_channels: EventHandler<()>,
    on_retry_messages: EventHandler<()>,
) -> Element {
    let channel_list = channels.loaded().cloned().unwrap_or_default();
    // Only a loaded list has a count. While loading or after an error the header
    // would otherwise read "Channels (0)" directly above the spinner or error.
    let count = channels.loaded().map(|c| c.len());
    // Listener count for the selected channel, from the loaded list rather than
    // a literal, so the toolbar cannot disagree with the sidebar.
    let listeners = selected.as_ref().and_then(|name| {
        channel_list
            .iter()
            .find(|c| &c.name == name)
            .map(|c| c.subscribers.clone())
    });
    // The seam returns the whole tail; scope it to the selection here so
    // picking a channel means something. A channel with no traffic renders an
    // empty tail, which is the truth, not a blank pane.
    let rows: Vec<NotifyMessage> = messages
        .loaded()
        .map(|all| {
            all.iter()
                .filter(|m| selected.as_ref().is_none_or(|s| &m.channel == s))
                .cloned()
                .collect()
        })
        .unwrap_or_default();

    rsx! {
        div { style: "display: grid; grid-template-columns: 260px 1fr; overflow: hidden;",
            div { style: "background: var(--bg-secondary); border-right: 0.5px solid var(--border-mid); padding: 10px;",
                div { class: "eyebrow", style: "padding: 6px 10px;",
                    if let Some(n) = count { "Channels ({n})" } else { "Channels" }
                }
                AsyncView {
                    loading: channels.is_loading(),
                    empty: channels.is_empty(),
                    error: channels.error_message(),
                    retriable: channels.is_retriable(),
                    on_retry: move |_| on_retry_channels.call(()),
                    empty_message: "No channels.".to_string(),
                }
                for c in channel_list {
                    div {
                        key: "{c.id}",
                        class: if selected.as_deref() == Some(c.name.as_str()) { "collection active" } else { "collection" },
                        onclick: {
                            let name = c.name.clone();
                            move |_| on_pick.call(name.clone())
                        },
                        span { class: "ico", "#" }
                        " {c.name} "
                        span { class: "count", "{c.subscribers}" }
                    }
                }
            }
            div { class: "live-tail",
                div { class: "tail-toolbar",
                    strong { style: "font-size:13px;", "{selected.clone().unwrap_or_default()}" }
                    if let Some(n) = listeners {
                        span { class: "pill info", span { class: "dot" } "{n} listeners" }
                    }
                    div { style: "margin-left:auto; display:flex; gap:6px;",
                        input {
                            placeholder: "payload filter",
                            style: "padding:4px 8px; background: var(--bg-primary); border: 0.5px solid var(--border-mid); border-radius: 4px; font-family: var(--font-mono); font-size: 11px;",
                        }
                        button { class: "btn small", "Send NOTIFY" }
                    }
                }
                div { class: "tail-body",
                    AsyncView {
                        loading: messages.is_loading(),
                        // Driven by the FILTERED rows, not the raw read: the seam
                        // returns every channel's messages, so a channel with no
                        // traffic has a non-empty read and zero rows. Keying this
                        // off `messages.is_empty()` renders the blank pane this
                        // screen exists to avoid. Loading and Error win, so the
                        // spinner and the error are not replaced by "No messages".
                        empty: rows.is_empty()
                            && !messages.is_loading()
                            && messages.error_message().is_none(),
                        error: messages.error_message(),
                        retriable: messages.is_retriable(),
                        on_retry: move |_| on_retry_messages.call(()),
                        empty_message: "No messages.".to_string(),
                    }
                    for m in rows {
                        div { key: "{m.id}", class: "tail-row",
                            span { class: "time", "{m.at}" }
                            span { class: "op ins", "NOTIFY" }
                            span { class: "coll", "{m.channel}" }
                            span { class: "payload", "{m.payload_json}" }
                        }
                    }
                }
                div { class: "tail-footer",
                    span { class: "tail-pulse" }
                    span { "following" }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::error::StudioError;

    fn render(app: fn() -> Element) -> String {
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        dioxus_ssr::render(&dom)
    }

    fn channel(id: &str, name: &str, subs: &str) -> NotifyChannel {
        NotifyChannel {
            id: id.into(),
            name: name.into(),
            subscribers: subs.into(),
        }
    }

    fn message(id: &str, chan: &str) -> NotifyMessage {
        NotifyMessage {
            id: id.into(),
            channel: chan.into(),
            at: "04:23:18.041".into(),
            payload_json: "{\"event\":\"login\"}".into(),
        }
    }

    fn sample_channels() -> Vec<NotifyChannel> {
        vec![
            channel("channel-1", "user_events", "12"),
            channel("channel-2", "deploy_hooks", "3"),
        ]
    }

    #[test]
    fn default_channel_is_the_first_one_the_seam_returned() {
        assert_eq!(
            default_channel(&sample_channels()).as_deref(),
            Some("user_events")
        );
    }

    #[test]
    fn default_channel_is_none_when_there_are_no_channels() {
        assert!(default_channel(&[]).is_none());
    }

    fn app_loading() -> Element {
        rsx! {
            NotifyPanes {
                channels: AsyncState::Loading,
                messages: AsyncState::Loading,
                selected: None,
                on_pick: move |_| {},
                on_retry_channels: move |_| {},
                on_retry_messages: move |_| {},
            }
        }
    }

    fn app_empty() -> Element {
        rsx! {
            NotifyPanes {
                channels: AsyncState::from_value(Some(Ok(Vec::<NotifyChannel>::new()))),
                messages: AsyncState::from_value(Some(Ok(Vec::<NotifyMessage>::new()))),
                selected: None,
                on_pick: move |_| {},
                on_retry_channels: move |_| {},
                on_retry_messages: move |_| {},
            }
        }
    }

    fn app_error() -> Element {
        rsx! {
            NotifyPanes {
                channels: AsyncState::Error(StudioError::NotConnected),
                messages: AsyncState::Error(StudioError::NotConnected),
                selected: None,
                on_pick: move |_| {},
                on_retry_channels: move |_| {},
                on_retry_messages: move |_| {},
            }
        }
    }

    fn app_ready() -> Element {
        rsx! {
            NotifyPanes {
                channels: AsyncState::from_value(Some(Ok(sample_channels()))),
                messages: AsyncState::from_value(Some(Ok(vec![
                    message("notify-0", "user_events"),
                    message("notify-1", "deploy_hooks"),
                ]))),
                selected: Some("user_events".to_string()),
                on_pick: move |_| {},
                on_retry_channels: move |_| {},
                on_retry_messages: move |_| {},
            }
        }
    }

    #[test]
    fn loading_state_renders_spinner() {
        assert!(render(app_loading).contains("async-loading"));
    }

    #[test]
    fn empty_state_renders_message() {
        let html = render(app_empty);
        assert!(html.contains("No channels."));
        assert!(html.contains("No messages."));
    }

    #[test]
    fn error_state_renders_error() {
        assert!(render(app_error).contains("async-error"));
    }

    #[test]
    fn ready_state_renders_keyed_channels_and_marks_the_selection() {
        let html = render(app_ready);
        assert!(html.contains("user_events"));
        assert!(html.contains("deploy_hooks"));
        assert!(
            html.contains("collection active"),
            "the selected channel must be marked: {html}"
        );
    }

    /// The tail is scoped to the selection. A message on another channel must
    /// not leak into the pane, or picking a channel means nothing.
    #[test]
    fn tail_shows_only_the_selected_channels_messages() {
        let html = render(app_ready);
        assert_eq!(
            html.matches("tail-row").count(),
            1,
            "only the user_events message belongs in the tail: {html}"
        );
    }

    /// A channel with no traffic must say so. The seam returns every channel's
    /// messages, so the read is non-empty while the filtered tail has zero
    /// rows; keying the empty flag off the raw read renders a blank pane.
    fn app_quiet_channel() -> Element {
        rsx! {
            NotifyPanes {
                channels: AsyncState::from_value(Some(Ok(sample_channels()))),
                messages: AsyncState::from_value(Some(Ok(vec![message(
                    "notify-0",
                    "user_events",
                )]))),
                selected: Some("deploy_hooks".to_string()),
                on_pick: move |_| {},
                on_retry_channels: move |_| {},
                on_retry_messages: move |_| {},
            }
        }
    }

    #[test]
    fn quiet_channel_says_no_messages_instead_of_rendering_blank() {
        let html = render(app_quiet_channel);
        assert!(html.contains("No messages."), "{html}");
        assert_eq!(html.matches("tail-row").count(), 0);
    }

    /// Loading and Error must win over the filtered-empty check, or the
    /// spinner and the error text get replaced by "No messages."
    fn app_loading_tail() -> Element {
        rsx! {
            NotifyPanes {
                channels: AsyncState::from_value(Some(Ok(sample_channels()))),
                messages: AsyncState::Loading,
                selected: Some("user_events".to_string()),
                on_pick: move |_| {},
                on_retry_channels: move |_| {},
                on_retry_messages: move |_| {},
            }
        }
    }

    #[test]
    fn loading_tail_shows_the_spinner_not_no_messages() {
        let html = render(app_loading_tail);
        assert!(html.contains("async-loading"), "{html}");
        assert!(!html.contains("No messages."), "{html}");
    }

    /// The header must not claim a count while the list is still loading.
    #[test]
    fn header_omits_the_count_until_channels_load() {
        let html = render(app_loading);
        assert!(html.contains("Channels"), "{html}");
        assert!(!html.contains("Channels (0)"), "{html}");
    }

    /// The toolbar's listener count comes from the loaded channel list, so it
    /// cannot disagree with the number rendered beside the same channel in the
    /// sidebar.
    #[test]
    fn listener_count_comes_from_the_loaded_channel() {
        let html = render(app_ready);
        assert!(html.contains("12 listeners"), "{html}");
    }
}
