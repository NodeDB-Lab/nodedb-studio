//! Preferences modal body: a sidebar of categories + the active pane.
//!
//! App-level only (theme, editor, keyboard, security, telemetry, about) — never
//! per-connection. The Theme control is wired to the `Preferences` signal; the
//! remaining controls are visual placeholders, as in the mockup. Note the CSS
//! port only defines OS-driven theming, so changing Theme updates state without
//! a manual override repaint (faithful to the mockup, which is also static).

use dioxus::prelude::*;

use crate::state::preferences::{Preferences, Theme};
use crate::state::ui::ModalKind;

const CATS: [(&str, &str); 6] = [
    ("appearance", "Appearance"),
    ("editor", "Editor"),
    ("keyboard", "Keyboard"),
    ("security", "Security"),
    ("telemetry", "Telemetry"),
    ("about", "About"),
];

#[component]
pub fn PreferencesPanes() -> Element {
    let mut modal = use_context::<Signal<Option<ModalKind>>>();
    let mut pane = use_signal(|| "appearance".to_string());
    let current = pane.read().clone();

    rsx! {
        div { class: "prefs-layout",
            div { class: "prefs-sidebar",
                for (key, label) in CATS {
                    {
                        let is_active = current == key;
                        let k = key.to_string();
                        rsx! {
                            div {
                                class: if is_active { "prefs-cat active" } else { "prefs-cat" },
                                onclick: move |_| pane.set(k.clone()),
                                "{label}"
                            }
                        }
                    }
                }
            }
            div { class: "prefs-content",
                match current.as_str() {
                    "editor" => rsx! { EditorPane {} },
                    "keyboard" => rsx! { KeyboardPane {} },
                    "security" => rsx! { SecurityPane {} },
                    "telemetry" => rsx! { TelemetryPane {} },
                    "about" => rsx! { AboutPane {} },
                    _ => rsx! { AppearancePane {} },
                }
            }
        }
        div { class: "modal-footer",
            button { class: "btn primary", onclick: move |_| modal.set(None), "Done" }
        }
    }
}

#[component]
fn AppearancePane() -> Element {
    let mut prefs = use_context::<Signal<Preferences>>();
    let theme = prefs.read().theme;
    let cls = |t: Theme| if theme == t { "active" } else { "" };
    rsx! {
        div { class: "prefs-pane active",
            h2 { "Appearance" }
            p { "How the studio looks. Follows your OS theme by default." }
            div { class: "form-field",
                label { "Theme" }
                div { class: "segmented", style: "display:flex;",
                    button { class: cls(Theme::Light), onclick: move |_| prefs.write().theme = Theme::Light, "Light" }
                    button { class: cls(Theme::Dark), onclick: move |_| prefs.write().theme = Theme::Dark, "Dark" }
                    button { class: cls(Theme::System), onclick: move |_| prefs.write().theme = Theme::System, "Follow OS" }
                }
            }
            div { class: "form-field",
                label { "Density" }
                div { class: "segmented", style: "display:flex;",
                    button { "Compact" } button { class: "active", "Comfortable" } button { "Spacious" }
                }
            }
            div { class: "form-row",
                div { class: "form-field", label { "UI font size" } select { option { "12px" } option { "13px (default)" } option { "14px" } } }
                div { class: "form-field", label { "Editor font" } select { option { "JetBrains Mono" } option { "SF Mono" } option { "Menlo" } } }
            }
            div { class: "form-field",
                label { "Accent color" }
                div { style: "display:flex; gap:8px;",
                    div { style: "width:24px; height:24px; background:#1a1a18; border-radius:4px; outline: 2px solid var(--text-primary); outline-offset: 2px;" }
                    div { style: "width:24px; height:24px; background:#185fa5; border-radius:4px;" }
                    div { style: "width:24px; height:24px; background:#3b6d11; border-radius:4px;" }
                    div { style: "width:24px; height:24px; background:#854f0b; border-radius:4px;" }
                }
            }
        }
    }
}

#[component]
fn EditorPane() -> Element {
    rsx! {
        div { class: "prefs-pane active",
            h2 { "Editor" }
            p { "Code editor behavior across SQL, Cypher, and the REPL." }
            div { class: "form-row",
                div { class: "form-field", label { "Tab size" } select { option { "2 spaces" } option { "4 spaces (default)" } option { "Tab" } } }
                div { class: "form-field", label { "Word wrap" } select { option { "Off" } option { "On" } } }
            }
            div { class: "form-field", label { "Auto-format on save" } div { class: "segmented", style: "display:flex;", button { "Off" } button { class: "active", "On" } } }
            div { class: "form-field", label { "Show whitespace" } div { class: "segmented", style: "display:flex;", button { class: "active", "Off" } button { "On" } } }
            div { class: "form-field", label { "Bracket pair colorization" } div { class: "segmented", style: "display:flex;", button { "Off" } button { class: "active", "On" } } }
        }
    }
}

#[component]
fn KeyboardPane() -> Element {
    let rows = [
        ("Command palette", "⌘K"),
        ("Disconnect", "⌘D"),
        ("Run query", "⌘↵"),
        ("Open preferences", "⌘,"),
        ("New query tab", "⌘T"),
        ("Toggle theme", "⌘⇧L"),
        ("Focus Explorer sidebar", "⌘B"),
    ];
    rsx! {
        div { class: "prefs-pane active",
            h2 { "Keyboard shortcuts" }
            p { "Customize bindings. Defaults shown." }
            div { style: "font-family: var(--font-mono); font-size: 12px;",
                for (cmd, key) in rows {
                    div { style: "display: grid; grid-template-columns: 1fr auto; padding: 8px 0; border-bottom: 0.5px solid var(--border-soft);",
                        span { "{cmd}" }
                        span { "{key}" }
                    }
                }
            }
        }
    }
}

#[component]
fn SecurityPane() -> Element {
    rsx! {
        div { class: "prefs-pane active",
            h2 { "Security" }
            p { "Credential storage and session protections. These are app-level — per-connection auth settings live with each connection." }
            div { class: "form-field", label { "Credential storage" } select { option { "OS keychain (default)" } option { "Encrypted file" } option { "In-memory only" } } }
            div { class: "form-field", label { "Lock app after idle" } select { option { "Never" } option { "5 minutes" } option { "15 minutes" } option { "1 hour" } } }
            div { class: "form-field", label { "Confirm destructive queries" } div { class: "segmented", style: "display:flex;", button { "Off" } button { class: "active", "On" } } }
            div { class: "form-field", label { "Mask sensitive query results" } div { class: "segmented", style: "display:flex;", button { class: "active", "Off" } button { "On" } } }
        }
    }
}

#[component]
fn TelemetryPane() -> Element {
    rsx! {
        div { class: "prefs-pane active",
            h2 { "Telemetry" }
            p { "Anonymous usage data helps improve NodeDB-Studio. Off by default." }
            div { class: "form-field", label { "Send anonymous usage data" } div { class: "segmented", style: "display:flex;", button { class: "active", "Off" } button { "On" } } }
            div { class: "form-field", label { "Send crash reports" } div { class: "segmented", style: "display:flex;", button { "Off" } button { class: "active", "On" } } }
            div { style: "font-family: var(--font-mono); font-size: 11px; color: var(--text-tertiary); margin-top: 10px; line-height: 1.6;", "No connection data, credentials, or query content is ever transmitted. Only anonymized UI interactions and error stack traces." }
        }
    }
}

#[component]
fn AboutPane() -> Element {
    rsx! {
        div { class: "prefs-pane active",
            h2 { "About NodeDB-Studio" }
            p { "Database studio built on Dioxus + Rust." }
            div { style: "font-family: var(--font-mono); font-size: 11px; line-height: 1.8; margin-top: 14px;",
                AboutRow { k: "version", v: "dev" }
                AboutRow { k: "build", v: "2026.06.13" }
                AboutRow { k: "runtime", v: "dioxus 0.7" }
                AboutRow { k: "platform", v: "darwin-arm64" }
                AboutRow { k: "license", v: "Apache 2.0" }
            }
            div { style: "margin-top: 18px; display: flex; gap: 8px;",
                button { class: "btn small", "Check for updates" }
                button { class: "btn small", "Release notes" }
                button { class: "btn small", "View license" }
            }
        }
    }
}

#[component]
fn AboutRow(k: String, v: String) -> Element {
    rsx! {
        div {
            span { style: "color:var(--text-tertiary); display:inline-block; width:80px;", "{k}" }
            "{v}"
        }
    }
}
