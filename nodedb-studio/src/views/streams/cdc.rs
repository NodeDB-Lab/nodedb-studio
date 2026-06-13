//! Streams · CDC: a live tail of change events. Payloads are native documents
//! (see `data::mock`); each is serialized to JSON here, at the view, for
//! display — the same seam the real Explorer will use on `Value`s from the
//! client.

use dioxus::prelude::*;

use crate::data::mock;

#[component]
pub fn StreamsCdc() -> Element {
    // (time, op-label, op-css, collection, payload-json)
    let rows: Vec<(&str, &str, &str, &str, String)> = mock::cdc_events()
        .into_iter()
        .map(|ev| {
            let mut payload = sonic_rs::to_string(&ev.payload).unwrap_or_default();
            if let Some(note) = ev.note {
                payload.push(' ');
                payload.push_str(note);
            }
            (ev.time, ev.op.label(), ev.op.css(), ev.collection, payload)
        })
        .collect();
    rsx! {
        div { class: "live-tail",
            div { class: "tail-toolbar",
                strong { style: "font-size:13px;", "events_cdc" }
                span { class: "pill ok", span { class: "dot" } "live · 2,103 ev/s" }
                div { style: "margin-left:auto; display:flex; gap:6px; align-items:center;",
                    input {
                        placeholder: "filter: type=signup",
                        style: "padding:4px 8px; background: var(--bg-primary); border: 0.5px solid var(--border-mid); border-radius: 4px; font-family: var(--font-mono); font-size: 11px;",
                    }
                    button { class: "btn small", "Pause" }
                    button { class: "btn small", "⇣ Export" }
                }
            }
            div { class: "tail-body",
                for r in rows {
                    div { class: "tail-row",
                        span { class: "time", "{r.0}" }
                        span { class: "op {r.2}", "{r.1}" }
                        span { class: "coll", "{r.3}" }
                        span { class: "payload", "{r.4}" }
                    }
                }
            }
            div { class: "tail-footer",
                span { class: "tail-pulse" }
                span { "following tail" }
                span { "buffer: 9 / 5,000" }
                span { "lag from leader: 12 ms" }
                span { style: "margin-left:auto;", "columns: time, op, collection, payload" }
            }
        }
    }
}
