//! The Editor, Keyboard shortcuts, Security, Telemetry, and About panes.

use dioxus::prelude::*;

use crate::modals::preferences::controls::{PrefRow, PrefSelect, PrefSwitch};
use crate::models::shell::APP_VERSION;
use crate::state::preferences::{Preferences, TabSize};

#[component]
pub fn EditorPane(draft: Signal<Preferences>) -> Element {
    let mut draft = draft;
    let p = draft.read().clone();
    rsx! {
        h2 { class: "pane-title", "Editor" }
        p { class: "pane-lede", "How the Query and Console editors lay out text." }
        PrefRow { label: "Tab size", hint: "Spaces per tab stop.",
            PrefSelect {
                label: "Tab size",
                options: TabSize::ALL
                    .iter()
                    .map(|t| (t.width().to_string(), format!("{} spaces", t.width())))
                    .collect(),
                value: p.tab_size.width().to_string(),
                on_pick: move |key: String| {
                    if let Some(t) = TabSize::ALL.iter().find(|t| t.width().to_string() == key) {
                        draft.write().tab_size = *t;
                    }
                },
            }
        }
        PrefRow { label: "Word wrap", hint: "Wrap long lines instead of scrolling sideways.",
            PrefSwitch {
                label: "Word wrap",
                checked: p.word_wrap,
                on_change: move |on| draft.write().word_wrap = on,
            }
        }
        p { class: "pane-note", "The editor font is under Appearance." }
    }
}

/// Shortcuts the app handles. The list matches the key handler in `App`.
const SHORTCUTS: [(&str, &str); 4] = [
    ("Command palette", "Cmd K"),
    ("Open preferences", "Cmd ,"),
    ("Disconnect", "Cmd D"),
    ("Close the open menu, drawer, or dialog", "Esc"),
];

#[component]
pub fn KeyboardPane() -> Element {
    rsx! {
        h2 { class: "pane-title", "Keyboard shortcuts" }
        p { class: "pane-lede", "On Windows and Linux, Cmd is the Ctrl key." }
        div { class: "kv shortcuts",
            for (action, keys) in SHORTCUTS {
                div { key: "{action}", class: "kv-row",
                    span { "{action}" }
                    kbd { "{keys}" }
                }
            }
        }
        p { class: "pane-note", "Rebinding keys is not designed yet." }
    }
}

#[component]
pub fn SecurityPane(draft: Signal<Preferences>) -> Element {
    let mut draft = draft;
    let p = draft.read().clone();
    rsx! {
        h2 { class: "pane-title", "Security" }
        p { class: "pane-lede", "Protections that apply to this machine." }
        PrefRow { label: "Confirm destructive statements", hint: "Ask before a statement that deletes or drops data.",
            PrefSwitch {
                label: "Confirm destructive statements",
                checked: p.confirm_destructive,
                on_change: move |on| draft.write().confirm_destructive = on,
            }
        }
        PrefRow { label: "Credential storage", hint: "Where saved passwords and tokens live.",
            span { class: "pref-value", "OS keychain" }
        }
        p { class: "pane-note", "Studio never shows a saved password or token, and Test connection never saves one." }
    }
}

#[component]
pub fn TelemetryPane(draft: Signal<Preferences>) -> Element {
    let mut draft = draft;
    let p = draft.read().clone();
    rsx! {
        h2 { class: "pane-title", "Telemetry" }
        p { class: "pane-lede", "Anonymous usage data helps improve NodeDB Studio. It is off by default." }
        PrefRow { label: "Send anonymous usage data", hint: "Interface events and error traces only.",
            PrefSwitch {
                label: "Send anonymous usage data",
                checked: p.telemetry,
                on_change: move |on| draft.write().telemetry = on,
            }
        }
        p { class: "pane-note", "Studio never sends connection data, credentials, or query text." }
    }
}

#[component]
pub fn AboutPane() -> Element {
    let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
    rsx! {
        h2 { class: "pane-title", "About NodeDB Studio" }
        p { class: "pane-lede", "A desktop client for NodeDB, built with Rust and Dioxus." }
        div { class: "kv",
            div { class: "kv-row", span { "Version" } span { class: "mono", "{APP_VERSION}" } }
            div { class: "kv-row", span { "Platform" } span { class: "mono", "{platform}" } }
            div { class: "kv-row", span { "License" } span { class: "mono", "Apache-2.0" } }
        }
    }
}
