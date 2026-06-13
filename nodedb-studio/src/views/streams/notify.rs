//! Streams · LISTEN/NOTIFY: channel list + a live pub/sub tail. Payloads are
//! native documents (see `data::mock`), serialized to JSON here for display.

use dioxus::prelude::*;

use crate::data::mock;

#[component]
pub fn StreamsNotify() -> Element {
    let channels = mock::notify_channels();
    // (time, source, payload-json)
    let rows: Vec<(&str, &str, String)> = mock::notify_messages()
        .into_iter()
        .map(|m| {
            (
                m.time,
                m.source,
                sonic_rs::to_string(&m.payload).unwrap_or_default(),
            )
        })
        .collect();
    rsx! {
        div { style: "display: grid; grid-template-columns: 260px 1fr; overflow: hidden;",
            div { style: "background: var(--bg-secondary); border-right: 0.5px solid var(--border-mid); padding: 10px;",
                div { class: "eyebrow", style: "padding: 6px 10px;", "Channels (14)" }
                for c in channels {
                    div { class: if c.active { "collection active" } else { "collection" },
                        span { class: "ico", "#" }
                        " {c.name} "
                        span { class: "count", "{c.listeners}" }
                    }
                }
            }
            div { class: "live-tail",
                div { class: "tail-toolbar",
                    strong { style: "font-size:13px;", "user_events" }
                    span { class: "pill info", span { class: "dot" } "12 listeners" }
                    div { style: "margin-left:auto; display:flex; gap:6px;",
                        input {
                            placeholder: "payload filter",
                            style: "padding:4px 8px; background: var(--bg-primary); border: 0.5px solid var(--border-mid); border-radius: 4px; font-family: var(--font-mono); font-size: 11px;",
                        }
                        button { class: "btn small", "Send NOTIFY" }
                    }
                }
                div { class: "tail-body",
                    for r in rows {
                        div { class: "tail-row",
                            span { class: "time", "{r.0}" }
                            span { class: "op ins", "NOTIFY" }
                            span { class: "coll", "{r.1}" }
                            span { class: "payload", "{r.2}" }
                        }
                    }
                }
                div { class: "tail-footer",
                    span { class: "tail-pulse" }
                    span { "following" }
                    span { "throughput: 84 msg/s" }
                    span { "since: 04:18:00" }
                }
            }
        }
    }
}
