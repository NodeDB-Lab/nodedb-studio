//! Pieces the Cluster tabs share.

use dioxus::prelude::*;

use crate::models::cluster::{Observer, SEQUENCER_GROUP_ID};

/// Why a panel names its node: one connection reaches one node.
const SEEN_BY: &str = "Studio reads the cluster through the node it connected to. Roles, leaders, terms, node states and circuit states are that node's opinion, and another node can hold a different one during a change.";

/// Whose view a panel shows.
#[component]
pub fn SeenBy(observer: Observer) -> Element {
    rsx! {
        span { class: "badge outline", title: SEEN_BY, "{observer.label()}" }
    }
}

/// A value the server does not report. Never a zero or a blank.
#[component]
pub fn NotReported(#[props(default)] why: String) -> Element {
    rsx! {
        span { class: "muted", title: "{why}", "Not reported" }
    }
}

/// `value`, or Not reported when the server does not report it.
pub fn or_not_reported<T>(value: Option<T>, show: impl FnOnce(T) -> String) -> Element {
    match value.map(show) {
        Some(text) => rsx! { "{text}" },
        None => rsx! { NotReported {} },
    }
}

/// A group id short enough for a table cell. NodeDB's sequencer id is a
/// 20-digit number (SQL prints it as `-4294967295`), so the cell names the
/// group and its title keeps the id.
#[component]
pub fn GroupId(id: String) -> Element {
    let text = if id == SEQUENCER_GROUP_ID.to_string() {
        "sequencer"
    } else {
        id.as_str()
    };
    rsx! {
        span { title: "Group {id}", "{text}" }
    }
}

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
