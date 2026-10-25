//! The Test connection button and what its last run found.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::state::connection_form::TestState;

/// The outcome line next to Test connection: nothing before a run, then the
/// progress, the pass, or the error. A pass is not a save: the hint under
/// the row says so.
#[component]
pub fn TestOutcome(test: TestState) -> Element {
    match test {
        TestState::Idle => rsx! {
            span { class: "test-hint", "Test checks the server. It saves nothing, not even the password." }
        },
        TestState::Running => rsx! {
            span { class: "test-hint", "Testing connection…" }
        },
        TestState::Passed(result) => {
            let tls = if result.tls_verified {
                "TLS verified"
            } else {
                "TLS not used"
            };
            rsx! {
                div { class: "callout ok test-result", role: "status",
                    Ico { icon: Icon::CircleCheck, size: 18 }
                    div {
                        div { class: "callout-title", "Connection successful" }
                        div { class: "callout-detail",
                            "{tls} · latency {result.latency_ms}ms · server {result.server_version}"
                        }
                    }
                }
            }
        }
        TestState::Failed(message) => rsx! {
            div { class: "callout err test-result", role: "alert",
                Ico { icon: Icon::CircleAlert, size: 18 }
                div {
                    div { class: "callout-title", "Connection failed" }
                    div { class: "callout-detail", "{message}" }
                }
            }
        },
    }
}
