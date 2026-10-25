//! The Appearance pane: theme, density, font size, editor font, motion,
//! contrast preview, and time zone. Every row edits the dialog's draft;
//! Save applies it to the app.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::modals::preferences::controls::{PrefRow, PrefSelect, PrefSwitch};
use crate::models::format::offset_label;
use crate::state::preferences::{Density, EditorFont, FontSize, Preferences, Theme, UTC_OFFSETS};

fn theme_icon(t: Theme) -> Icon {
    match t {
        Theme::Light => Icon::Sun,
        Theme::Dark => Icon::Moon,
        Theme::System => Icon::Monitor,
    }
}

/// The line under the theme cards. `os_dark` is what the webview reports
/// for the OS color scheme, `None` until it answers or when it cannot.
fn theme_hint(theme: Theme, os_dark: Option<bool>) -> String {
    match (theme, os_dark) {
        (Theme::System, Some(true)) => "Follows your system theme. Effective theme: Dark.".into(),
        (Theme::System, Some(false)) => "Follows your system theme. Effective theme: Light.".into(),
        (Theme::System, None) => "Follows your system theme.".into(),
        (Theme::Light | Theme::Dark, _) => format!("Always the {} theme.", theme.label()),
    }
}

fn options<T: Copy>(
    all: &[T],
    key: impl Fn(T) -> String,
    text: impl Fn(T) -> String,
) -> Vec<(String, String)> {
    all.iter().map(|t| (key(*t), text(*t))).collect()
}

#[component]
pub fn AppearancePane(draft: Signal<Preferences>) -> Element {
    let mut draft = draft;
    // What the OS prefers, asked of the webview. A read, so it is scoped to
    // the pane.
    let os_dark = use_resource(|| async {
        document::eval("return window.matchMedia('(prefers-color-scheme: dark)').matches")
            .join::<bool>()
            .await
            .ok()
    });
    let p = draft.read().clone();
    let os = os_dark.read().as_ref().copied().flatten();
    rsx! {
        h2 { class: "pane-title", "Appearance" }
        p { class: "pane-lede", "Customize the look and feel of NodeDB Studio." }
        PrefRow { label: "Theme", hint: "Choose the color theme for the application.",
            div { class: "choice-grid three", role: "radiogroup", "aria-label": "Theme",
                for t in Theme::ALL {
                    label {
                        key: "{t.attr()}",
                        class: if p.theme == t { "choice swatch selected" } else { "choice swatch" },
                        input {
                            r#type: "radio",
                            class: "radio",
                            name: "theme",
                            checked: p.theme == t,
                            onchange: move |_| draft.write().theme = t,
                        }
                        Ico { icon: theme_icon(t), size: 20 }
                        span { "{t.label()}" }
                    }
                }
            }
            div { class: "pref-note", "{theme_hint(p.theme, os)}" }
        }
        PrefRow { label: "Interface density", hint: "Adjust spacing and sizing of UI elements.",
            div { class: "choice-grid two", role: "radiogroup", "aria-label": "Interface density",
                for d in Density::ALL {
                    label {
                        key: "{d.attr()}",
                        class: if p.density == d { "choice swatch selected" } else { "choice swatch" },
                        input {
                            r#type: "radio",
                            class: "radio",
                            name: "density",
                            checked: p.density == d,
                            onchange: move |_| draft.write().density = d,
                        }
                        Ico { icon: Icon::List, size: 20 }
                        span { "{d.label()}" }
                    }
                }
            }
        }
        PrefRow { label: "Font size", hint: "Set the base font size for the interface.",
            PrefSelect {
                label: "Font size",
                options: options(&FontSize::ALL, |f| f.attr().to_string(), |f| f.px().to_string()),
                value: p.font_size.attr().to_string(),
                on_pick: move |key: String| {
                    if let Some(f) = FontSize::ALL.iter().find(|f| f.attr() == key) {
                        draft.write().font_size = *f;
                    }
                },
            }
        }
        PrefRow { label: "Editor font", hint: "Font used in code editors and query editors.",
            PrefSelect {
                label: "Editor font",
                options: options(&EditorFont::ALL, |f| f.attr().to_string(), |f| f.label().to_string()),
                value: p.editor_font.attr().to_string(),
                on_pick: move |key: String| {
                    if let Some(f) = EditorFont::ALL.iter().find(|f| f.attr() == key) {
                        draft.write().editor_font = *f;
                    }
                },
            }
        }
        PrefRow { label: "Reduce motion", hint: "Minimize animations and motion effects.",
            PrefSwitch {
                label: "Reduce motion",
                checked: p.reduce_motion,
                on_change: move |on| draft.write().reduce_motion = on,
            }
        }
        PrefRow { label: "Contrast preview", hint: "Ink, accent, border, and surface in the active theme.",
            div { class: "contrast-strip", "aria-hidden": "true",
                span { class: "swatch-ink" }
                span { class: "swatch-accent" }
                span { class: "swatch-border" }
                span { class: "swatch-surface" }
            }
        }
        PrefRow { label: "Timezone", hint: "Used for displaying all time and date values.",
            PrefSelect {
                label: "Timezone",
                options: options(&UTC_OFFSETS, |o| o.to_string(), offset_label),
                value: p.utc_offset_minutes.to_string(),
                on_pick: move |key: String| {
                    if let Some(o) = UTC_OFFSETS.iter().find(|o| o.to_string() == key) {
                        draft.write().utc_offset_minutes = *o;
                    }
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_theme_names_the_effective_one() {
        assert_eq!(
            theme_hint(Theme::System, Some(false)),
            "Follows your system theme. Effective theme: Light."
        );
        assert_eq!(
            theme_hint(Theme::System, Some(true)),
            "Follows your system theme. Effective theme: Dark."
        );
        assert_eq!(
            theme_hint(Theme::System, None),
            "Follows your system theme."
        );
        assert_eq!(
            theme_hint(Theme::Dark, Some(false)),
            "Always the Dark theme."
        );
    }
}
