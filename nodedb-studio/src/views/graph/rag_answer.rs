//! GraphRAG → Answer. An answer needs a configured provider. Without one
//! this tab says so and writes nothing.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};

#[component]
pub fn AnswerTab(provider: Option<String>, evidence: usize, question: String) -> Element {
    rsx! {
        div { class: "page-body",
            section { class: "panel fill", "aria-label": "Answer",
                div { class: "empty-state answer-state",
                    Ico { icon: Icon::Info, size: 28 }
                    match provider {
                        None => rsx! {
                            h3 { "An answer needs a configured provider" }
                            p {
                                "Retrieval is ready: {evidence} records for \"{question}\". "
                                "Studio does not write an answer from them until a provider is configured."
                            }
                        },
                        Some(name) => rsx! {
                            h3 { "Provider {name} is configured" }
                            p { "Answer generation from {evidence} retrieved records is not designed yet." }
                        },
                    }
                    div { class: "actions",
                        button {
                            class: "btn",
                            disabled: true,
                            title: "Provider setup is not designed yet",
                            "Configure provider"
                        }
                    }
                }
            }
        }
    }
}
