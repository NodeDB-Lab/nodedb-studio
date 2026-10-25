//! Pieces the Sync tabs share.

use dioxus::prelude::*;

use crate::models::sync::clock_ms;

/// A stage nobody reported. It is never filled from another stage.
pub fn unknown() -> Element {
    rsx! { span { class: "muted", "Unknown" } }
}

pub fn dash() -> Element {
    rsx! { span { class: "muted", "—" } }
}

/// One row of a stage time. A missing report reads Unknown.
#[component]
pub fn StageRow(label: String, ms: Option<i64>) -> Element {
    rsx! {
        div { class: "kv-row",
            span { "{label}" }
            match ms {
                Some(v) => rsx! { span { class: "num", "{clock_ms(v)}" } },
                None => unknown(),
            }
        }
    }
}
