//! A code editor built from a `textarea`: a line-number gutter, syntax
//! colors, and no dependency.
//!
//! A highlighted `pre` sits under a transparent `textarea` of the same
//! metrics. The two share one scroll container, so they never drift. Run
//! with Ctrl+Enter or Cmd+Enter.

use dioxus::prelude::*;

use crate::models::console::Protocol;
use crate::models::sql_text::tokenize;

/// The number of lines in `text`, counting a trailing empty one.
pub fn line_count(text: &str) -> usize {
    text.matches('\n').count() + 1
}

#[component]
pub fn CodeEditor(
    value: String,
    protocol: Protocol,
    on_input: EventHandler<String>,
    /// Ctrl+Enter or Cmd+Enter.
    #[props(default)]
    on_run: EventHandler<()>,
    /// Every other key, after the editor has seen it.
    #[props(default)]
    on_key: EventHandler<KeyboardEvent>,
    #[props(default)] placeholder: String,
    #[props(default)] label: String,
    #[props(default = false)] read_only: bool,
    #[props(default)] class: String,
) -> Element {
    let lines = line_count(&value);
    let tokens = tokenize(&value, protocol);
    rsx! {
        div { class: "code-input {class}",
            div { class: "code-scroll",
                div { class: "code-gutter", "aria-hidden": "true",
                    for n in 1..=lines {
                        span { key: "{n}", "{n}" }
                    }
                }
                div { class: "code-stack",
                    pre { class: "code-highlight", "aria-hidden": "true",
                        for (i, t) in tokens.iter().enumerate() {
                            span { key: "{i}", class: "{t.kind.class()}", "{t.text}" }
                        }
                        // A final newline would not take a line of its own.
                        "\n "
                    }
                    textarea {
                        class: "code-textarea",
                        "aria-label": "{label}",
                        placeholder: "{placeholder}",
                        spellcheck: "false",
                        autocomplete: "off",
                        wrap: "off",
                        readonly: read_only,
                        value: "{value}",
                        oninput: move |e| on_input.call(e.value()),
                        onkeydown: move |e| {
                            let meta = e.modifiers().ctrl() || e.modifiers().meta();
                            if meta && e.key() == Key::Enter {
                                e.prevent_default();
                                on_run.call(());
                            } else {
                                on_key.call(e);
                            }
                        },
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(app: fn() -> Element) -> String {
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        dioxus_ssr::render(&dom)
    }

    #[test]
    fn lines_count_newlines_plus_one() {
        assert_eq!(line_count(""), 1);
        assert_eq!(line_count("a"), 1);
        assert_eq!(line_count("a\nb"), 2);
        assert_eq!(line_count("a\n"), 2);
    }

    #[test]
    fn the_gutter_numbers_every_line_and_keywords_are_colored() {
        fn app() -> Element {
            rsx! {
                CodeEditor {
                    value: "SELECT 1\nFROM t".to_string(),
                    protocol: Protocol::Sql,
                    on_input: |_| {},
                }
            }
        }
        let html = render(app);
        assert!(html.contains("<span>1</span><span>2</span>"), "{html}");
        assert!(!html.contains("<span>3</span>"), "{html}");
        assert!(html.contains("class=\"tok-kw\">SELECT"), "{html}");
        assert!(html.contains("class=\"tok-num\">1"), "{html}");
    }
}
