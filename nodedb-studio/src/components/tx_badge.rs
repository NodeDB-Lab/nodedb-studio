//! The session's transaction state as a badge: Idle, Active (read-only),
//! Active (read-write), or Failed.

use dioxus::prelude::*;

use crate::models::workbench::TxState;

fn tone(state: TxState) -> (&'static str, &'static str) {
    match state {
        TxState::Idle => ("badge gray", "idle"),
        TxState::Active { .. } => ("badge green", "ok"),
        TxState::Failed => ("badge red", "err"),
    }
}

#[component]
pub fn TxBadge(state: TxState) -> Element {
    let (badge, dot) = tone(state);
    let label = state.label();
    rsx! {
        span { class: "{badge} tx-badge", title: "Transaction: {label}",
            span { class: "status-dot {dot}" }
            "{label}"
        }
    }
}
