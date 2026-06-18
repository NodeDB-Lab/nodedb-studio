//! Streams · CDC: a live tail of change events, read through the seam. Split into
//! a fetch wrapper (`StreamsCdc`, owns the async read) and a pure presentational
//! component (`CdcTail`, takes `AsyncState` as input) so the four states are
//! render-testable without a runtime.

use std::rc::Rc;
use std::time::Duration;

use dioxus::prelude::*;

use crate::components::async_view::AsyncView;
use crate::components::live_tail::LiveTail;
use crate::models::cdc::{CdcOp, CdcRow};
use crate::services::async_state::AsyncState;
use crate::services::backend::Backend;

/// Build a synthetic CDC row for the simulated stream. Pure (no clock) so it is
/// unit-testable; the streaming task supplies an incrementing `seq`.
pub fn synthetic_row(seq: u64) -> CdcRow {
    let op = match seq % 3 {
        0 => CdcOp::Insert,
        1 => CdcOp::Update,
        _ => CdcOp::Delete,
    };
    CdcRow {
        id: format!("cdc-live-{seq}"),
        time: format!("04:23:{:02}.{:03}", seq % 60, (seq * 37) % 1000),
        op,
        collection: "events".to_string(),
        payload_json: format!("{{\"seq\":{seq}}}"),
    }
}

#[component]
pub fn StreamsCdc() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let mut feed = use_resource(move || {
        let backend = backend.clone();
        async move { backend.cdc_feed().await }
    });

    let mut paused = use_signal(|| false);
    let mut live_rows = use_signal(Vec::<CdcRow>::new);

    use_future(move || async move {
        let mut seq: u64 = 0;
        loop {
            tokio::time::sleep(Duration::from_millis(1200)).await;
            if *paused.peek() {
                continue;
            }
            let row = synthetic_row(seq); // built before touching the signal
            seq += 1;
            let mut rows = live_rows.write();
            rows.insert(0, row);
            rows.truncate(200); // bounded buffer
            // guard `rows` drops here, before the next .await
        }
    });

    // Map the resource read into the canonical four-state value (clone out of the
    // guard — never hold it across an await; there is none here).
    let base = AsyncState::from_value(feed.read().clone());
    let state = base.project(|fetched: &Vec<CdcRow>| {
        let mut merged = live_rows.read().clone(); // clone out of the guard immediately
        merged.extend(fetched.iter().cloned());
        merged
    });

    rsx! {
        CdcTail {
            state,
            on_retry: move |_| feed.restart(),
            paused: *paused.read(),
            on_toggle_pause: move |_| {
                let now = *paused.peek();
                paused.set(!now);
            },
        }
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct CdcTailProps {
    pub state: AsyncState<Vec<CdcRow>>,
    #[props(default)]
    pub on_retry: EventHandler<()>,
    /// Whether the live stream is currently paused. Drives the button label.
    #[props(default)]
    pub paused: bool,
    /// Called when the user clicks the Pause/Resume button.
    #[props(default)]
    pub on_toggle_pause: EventHandler<()>,
}

#[component]
pub fn CdcTail(props: CdcTailProps) -> Element {
    let state = &props.state;
    let rows = state.loaded().cloned().unwrap_or_default();
    let pause_label = if props.paused { "Resume" } else { "Pause" };
    rsx! {
        AsyncView {
            loading: state.is_loading(),
            empty: state.is_empty(),
            error: state.error_message(),
            retriable: state.is_retriable(),
            on_retry: move |_| props.on_retry.call(()),
            empty_message: "No change events.".to_string(),
        }
        if state.loaded().is_some() {
            LiveTail {
                toolbar: rsx! {
                    strong { style: "font-size:13px;", "events_cdc" }
                    span { class: "pill ok", span { class: "dot" } "live · 2,103 ev/s" }
                    div { style: "margin-left:auto; display:flex; gap:6px; align-items:center;",
                        input {
                            placeholder: "filter: type=signup",
                            style: "padding:4px 8px; background: var(--bg-primary); border: 0.5px solid var(--border-mid); border-radius: 4px; font-family: var(--font-mono); font-size: 11px;",
                        }
                        button {
                            class: "btn small",
                            onclick: move |_| props.on_toggle_pause.call(()),
                            "{pause_label}"
                        }
                        button { class: "btn small", "⇣ Export" }
                    }
                },
                body: rsx! {
                    for row in rows.iter() {
                        div { key: "{row.id}", class: "tail-row",
                            span { class: "time", "{row.time}" }
                            span { class: "op {row.op.css()}", "{row.op.label()}" }
                            span { class: "coll", "{row.collection}" }
                            span { class: "payload", "{row.payload_json}" }
                        }
                    }
                },
                footer: rsx! {
                    span { class: "tail-pulse" }
                    span { "following tail" }
                    span { "buffer: {rows.len()} / 5,000" }
                    span { "lag from leader: 12 ms" }
                    span { style: "margin-left:auto;", "columns: time, op, collection, payload" }
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::cdc::CdcOp;
    use crate::services::error::StudioError;

    fn render(app: fn() -> Element) -> String {
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        dioxus_ssr::render(&dom)
    }

    fn sample_rows() -> Vec<CdcRow> {
        vec![CdcRow {
            id: "cdc-0".to_string(),
            time: "04:23:18.041".to_string(),
            op: CdcOp::Insert,
            collection: "events".to_string(),
            payload_json: "{\"_id\":\"evt\"}".to_string(),
        }]
    }

    fn app_loading() -> Element {
        rsx! { CdcTail { state: AsyncState::Loading } }
    }

    fn app_empty() -> Element {
        rsx! {
            CdcTail {
                state: AsyncState::from_value(Some(Ok(Vec::<CdcRow>::new()))),
            }
        }
    }

    fn app_error() -> Element {
        rsx! { CdcTail { state: AsyncState::Error(StudioError::NotConnected) } }
    }

    fn app_ready() -> Element {
        rsx! {
            CdcTail {
                state: AsyncState::from_value(Some(Ok(sample_rows()))),
            }
        }
    }

    #[test]
    fn loading_state_renders_spinner() {
        let html = render(app_loading);
        assert!(html.contains("async-loading"));
    }

    #[test]
    fn empty_state_renders_message() {
        let html = render(app_empty);
        assert!(html.contains("async-empty"));
        assert!(html.contains("No change events"));
    }

    #[test]
    fn error_state_renders_error() {
        let html = render(app_error);
        assert!(html.contains("async-error"));
    }

    #[test]
    fn ready_state_renders_keyed_rows() {
        let html = render(app_ready);
        assert!(html.contains("live-tail"));
        assert!(html.contains("INSERT"));
        assert!(html.contains("events"));
        assert!(html.contains("buffer: 1 / 5,000"));
    }

    #[test]
    fn synthetic_row_is_stable_and_keyed_by_seq() {
        let a = synthetic_row(7);
        assert_eq!(a.id, "cdc-live-7");
        assert!(!a.time.is_empty());
        // distinct seq -> distinct id (keeps list keys stable & unique)
        assert_ne!(synthetic_row(7).id, synthetic_row(8).id);
    }

    #[tokio::test]
    async fn stream_step_appends_without_guard_panic() {
        let mut rows: Vec<CdcRow> = Vec::new();
        // mimic one loop iteration: await first, then mutate — never a guard across await
        tokio::time::sleep(Duration::from_millis(1)).await;
        rows.insert(0, synthetic_row(0));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "cdc-live-0");
    }
}
