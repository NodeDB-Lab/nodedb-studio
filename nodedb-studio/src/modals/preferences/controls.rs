//! Controls shared by the preferences panes: a row with its label and
//! description, a switch, and a select over a typed set.

use dioxus::prelude::*;

/// One preference: its name and a line of description at the left, the
/// control at the right.
#[component]
pub fn PrefRow(label: &'static str, hint: &'static str, children: Element) -> Element {
    rsx! {
        div { class: "pref-row",
            div { class: "pref-text",
                div { class: "pref-label", "{label}" }
                div { class: "pref-hint", "{hint}" }
            }
            div { class: "pref-control", {children} }
        }
    }
}

#[component]
pub fn PrefSwitch(label: &'static str, checked: bool, on_change: EventHandler<bool>) -> Element {
    rsx! {
        input {
            r#type: "checkbox",
            class: "switch",
            role: "switch",
            "aria-label": "{label}",
            checked,
            onchange: move |e| on_change.call(e.checked()),
        }
    }
}

/// A select over typed options. `value` is each option's stable key. A pick
/// calls `on_pick` with the key of the chosen option.
#[component]
pub fn PrefSelect(
    label: &'static str,
    options: Vec<(String, String)>,
    value: String,
    on_pick: EventHandler<String>,
) -> Element {
    rsx! {
        select {
            class: "select native",
            "aria-label": "{label}",
            onchange: move |e| on_pick.call(e.value()),
            for (key, text) in options {
                option {
                    key: "{key}",
                    value: "{key}",
                    selected: key == value,
                    "{text}"
                }
            }
        }
    }
}
