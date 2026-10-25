//! The messages of one partition of the selected topic, read over a bounded
//! range of offsets.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::clipboard::use_copier;
use crate::components::icon::{Ico, Icon};
use crate::components::json_view::JsonView;
use crate::models::topics::PublishDraft;
use crate::models::topics::{MessageRange, Topic, TopicMessage, parse_range};
use crate::services::backend::Backend;
use crate::state::events::EventsUi;
use crate::state::events::feeds::Notice;
use crate::state::events::topics::PublishSheet;

/// The messages as JSON Lines: key, offset, timestamp, and the value as JSON.
/// This exports what is on screen, not the retained range.
pub fn export_lines(messages: &[TopicMessage]) -> String {
    #[derive(serde::Serialize)]
    struct Line<'a> {
        partition: u32,
        offset: u64,
        timestamp: &'a str,
        key: &'a str,
        value: sonic_rs::Value,
    }
    messages
        .iter()
        .filter_map(|m| {
            let value = sonic_rs::from_str::<sonic_rs::Value>(&m.value).ok()?;
            sonic_rs::to_string(&Line {
                partition: m.partition,
                offset: m.offset,
                timestamp: &m.timestamp,
                key: &m.key,
                value,
            })
            .ok()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[component]
pub fn MessagesPanel(topic: Topic) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut ui = use_context::<EventsUi>().topics;
    let copier = use_copier();
    let epoch = use_memo(move || ui.read().epoch);
    let (input, applied, selected_message, export_open) = {
        let u = ui.read();
        (
            u.input.clone(),
            u.applied,
            u.selected_message,
            u.export_open,
        )
    };

    let mut range = {
        let backend = backend.clone();
        let key = (topic.id.clone(), applied);
        use_resource(use_reactive!(|key| {
            let backend = backend.clone();
            let (_conn, _epoch) = (conn(), epoch());
            async move {
                let (id, (partition, from, to)) = key;
                backend.topic_messages(&id, partition, from, to).await
            }
        }))
    };
    let state = resource_state(&range);
    let loaded: Option<MessageRange> = state.loaded().cloned();
    let parsed = parse_range(&input.from, &input.to);
    let messages: Vec<TopicMessage> = loaded
        .as_ref()
        .map(|r| r.messages.clone())
        .unwrap_or_default();
    let detail: Option<TopicMessage> = selected_message.and_then(|(p, o)| {
        messages
            .iter()
            .find(|m| m.partition == p && m.offset == o)
            .cloned()
    });
    let export_text = export_lines(&messages);
    let export_count = messages.len();
    let topic_id = topic.id.clone();
    let topic_name = topic.name.clone();
    let partitions = topic.partitions;

    rsx! {
        div { class: "toolbar nowrap range-bar",
            label { class: "toolbar-label", r#for: "tm-partition", "Partition" }
            select {
                id: "tm-partition",
                class: "select native compact narrow",
                onchange: move |e| {
                    if let Ok(p) = e.value().parse::<u32>() {
                        ui.write().input.partition = p;
                    }
                },
                for p in 0..partitions {
                    option { key: "{p}", value: "{p}", selected: p == input.partition, "{p}" }
                }
            }
            label { class: "toolbar-label", r#for: "tm-from", "From offset" }
            input {
                id: "tm-from",
                class: if matches!(parsed, Err(crate::models::topics::RangeError::FromNotANumber)) { "input mono offset-input invalid" } else { "input mono offset-input" },
                inputmode: "numeric",
                value: "{input.from}",
                oninput: move |e| ui.write().input.from = e.value(),
            }
            label { class: "toolbar-label", r#for: "tm-to", "To offset" }
            input {
                id: "tm-to",
                class: if matches!(parsed, Err(crate::models::topics::RangeError::ToNotANumber)) { "input mono offset-input invalid" } else { "input mono offset-input" },
                inputmode: "numeric",
                value: "{input.to}",
                oninput: move |e| ui.write().input.to = e.value(),
                onkeydown: move |e| {
                    if e.key() != Key::Enter {
                        return;
                    }
                    let range = parse_range(&ui.peek().input.from, &ui.peek().input.to);
                    if let Ok((from, to)) = range {
                        let p = ui.peek().input.partition;
                        let mut w = ui.write();
                        w.applied = (p, from, to);
                        w.selected_message = None;
                    }
                },
            }
            button {
                class: "btn primary",
                disabled: parsed.is_err(),
                title: match &parsed {
                    Ok(_) => "Read at most {MAX_READ} offsets of this partition".to_string(),
                    Err(e) => e.to_string(),
                },
                onclick: move |_| {
                    let range = parse_range(&ui.peek().input.from, &ui.peek().input.to);
                    if let Ok((from, to)) = range {
                        let p = ui.peek().input.partition;
                        let mut w = ui.write();
                        w.applied = (p, from, to);
                        w.selected_message = None;
                    }
                },
                Ico { icon: Icon::Play, size: 16 }
                "Read"
            }
        }
        if let Err(e) = &parsed {
            div { class: "field-error range-error", role: "alert", "{e}" }
        }
        div { class: "toolbar nowrap summary-bar",
            span { class: "summary-text", role: "status",
                match loaded.as_ref() {
                    Some(r) => r.summary(),
                    None => String::new(),
                }
            }
            div { class: "toolbar-spacer" }
            div { class: "anchor",
                button {
                    class: "btn",
                    "aria-haspopup": "menu",
                    "aria-expanded": "{export_open}",
                    onclick: move |e| { e.stop_propagation(); ui.write().export_open = !export_open; },
                    Ico { icon: Icon::Download, size: 16 }
                    "Export"
                }
                if export_open {
                    div {
                        class: "popover align-right",
                        role: "menu",
                        onclick: move |e| e.stop_propagation(),
                        onkeydown: move |e| { if e.key() == Key::Escape { ui.write().export_open = false; } },
                        button {
                            class: "menu-item",
                            role: "menuitem",
                            disabled: export_count == 0,
                            onclick: {
                                let text = export_text.clone();
                                move |_| {
                                    copier.copy(text.clone());
                                    let mut w = ui.write();
                                    w.export_open = false;
                                    w.notice = Some(Notice::ok(format!("Copied {export_count} displayed messages as JSON Lines.")));
                                }
                            },
                            Ico { icon: Icon::Copy, size: 16 }
                            "Copy displayed messages ({export_count})"
                        }
                        button {
                            class: "menu-item",
                            role: "menuitem",
                            disabled: true,
                            title: "Exporting a retained range as a job needs a server transport that is not designed yet",
                            Ico { icon: Icon::Upload, size: 16 }
                            "Export the retained range as a job…"
                        }
                    }
                }
            }
            button {
                class: "btn",
                title: "Publish a message to {topic_name}",
                onclick: {
                    let topic_id = topic_id.clone();
                    move |_| {
                        ui.write().publish = Some(PublishSheet {
                            topic: topic_id.clone(),
                            draft: PublishDraft::blank(),
                            busy: false,
                            error: None,
                        });
                    }
                },
                Ico { icon: Icon::Send, size: 16 }
                "Publish"
            }
        }
        AsyncView {
            loading: state.is_loading(),
            empty: false,
            error: state.error_message(),
            retriable: state.is_retriable(),
            on_retry: move |_| range.restart(),
        }
        if let Some(r) = loaded.clone() {
            if r.messages.is_empty() {
                div { class: "async-empty",
                    "No retained message in offsets {r.from}–{r.to}. Partition {r.partition} retains offsets {r.log_start}–{r.log_end}."
                }
            } else {
                div { class: "dt-wrap",
                    table { class: "dt messages-table",
                        thead {
                            tr {
                                th { "Offset" }
                                th { "Timestamp (UTC)" }
                                th { "Key" }
                                th { "Value" }
                            }
                        }
                        tbody {
                            for m in r.messages.iter() {
                                {
                                    let at = (m.partition, m.offset);
                                    let is_selected = selected_message == Some(at);
                                    rsx! {
                                        tr {
                                            key: "{m.partition}-{m.offset}",
                                            class: if is_selected { "clickable selected" } else { "clickable" },
                                            "aria-selected": "{is_selected}",
                                            tabindex: "0",
                                            onclick: move |_| ui.write().selected_message = Some(at),
                                            onkeydown: move |e| {
                                                if e.key() == Key::Enter {
                                                    ui.write().selected_message = Some(at);
                                                }
                                            },
                                            td { class: "mono", "{m.offset}" }
                                            td { class: "mono", "{m.timestamp}" }
                                            td { class: "mono", "{m.key}" }
                                            td { class: "mono value-cell", title: "{m.value}", "{m.value}" }
                                        }
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
                    strong { "Offset {m.offset}" }
                    span { class: "muted", "partition {m.partition} · key {m.key} · {m.timestamp} UTC" }
                }
                JsonView { text: m.value.clone() }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::MockConnectionService;
    use crate::services::topic_data::TopicData;

    #[tokio::test]
    async fn the_export_is_one_compact_object_per_message() {
        let r = MockConnectionService::ready()
            .topic_messages("topic_01", 1, 100, 104)
            .await
            .expect("reads");
        let out = export_lines(&r.messages);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 5);
        assert!(lines[0].contains("\"offset\":104"), "{}", lines[0]);
        assert!(lines[0].contains("\"orderId\":\"o104\""), "{}", lines[0]);
        assert_eq!(export_lines(&[]), "");
    }
}
