//! Pieces the Cluster tabs share.

use dioxus::prelude::*;

/// A toolbar control: its label, then the control. The label names the
/// control for assistive technology.
#[component]
pub fn Ctl(label: String, id: String, children: Element) -> Element {
    rsx! {
        div { class: "cl-ctl",
            label { r#for: "{id}", "{label}" }
            {children}
        }
    }
}

/// One fact: a label and its value, for the three-column fact grids.
#[component]
pub fn Fact(label: String, children: Element) -> Element {
    rsx! {
        div { class: "cl-fact",
            span { class: "cl-fact-label", "{label}" }
            span { class: "cl-fact-value", {children} }
        }
    }
}

/// A short list of names as chips.
#[component]
pub fn NameChips(names: Vec<String>, #[props(default)] highlight: Option<String>) -> Element {
    rsx! {
        span { class: "cl-chips",
            for n in names {
                span {
                    key: "{n}",
                    class: if highlight.as_deref() == Some(n.as_str()) { "chip active" } else { "chip" },
                    "{n}"
                }
            }
        }
    }
}
