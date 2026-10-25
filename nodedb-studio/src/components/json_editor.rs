//! A JSON text editor with line numbers and colors, and a state line that
//! says whether the text parses.
//!
//! The textarea sits over a colored copy of the same text, so the colors
//! follow every keystroke. The classes are the Explorer record editor's.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::json_text::json_issue;
use crate::models::record_json::tokens;

#[component]
pub fn JsonEditor(
    text: String,
    label: String,
    on_input: EventHandler<String>,
    #[props(default = false)] read_only: bool,
) -> Element {
    let lines = text.lines().count().max(1);
    rsx! {
        div { class: "json-editor",
            div { class: "gutter", "aria-hidden": "true",
                for n in 1..=lines {
                    div { key: "{n}", "{n}" }
                }
            }
            div { class: "editor-stack",
                pre { class: "highlight", "aria-hidden": "true",
                    for (i, line) in text.lines().enumerate() {
                        div { key: "{i}",
                            for (j, (kind, part)) in tokens(line).into_iter().enumerate() {
                                span { key: "{j}", class: "{kind.class()}", "{part}" }
                            }
                            if line.is_empty() { " " }
                        }
                    }
                }
                textarea {
                    class: "input-layer",
                    spellcheck: false,
                    wrap: "off",
                    readonly: read_only,
                    "aria-label": "{label}",
                    value: "{text}",
                    oninput: move |e| on_input.call(e.value()),
                }
            }
        }
    }
}

/// `JSON valid`, or why the text does not parse.
#[component]
pub fn JsonState(text: String) -> Element {
    match json_issue(&text) {
        None => rsx! {
            div { class: "json-state ok", role: "status",
                Ico { icon: Icon::CircleCheck, size: 16 }
                "JSON valid"
            }
        },
        Some(why) => rsx! {
            div { class: "json-state err", role: "alert",
                Ico { icon: Icon::CircleAlert, size: 16 }
                "{why}"
            }
        },
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
    fn valid_json_says_so_and_invalid_json_says_why() {
        fn ok() -> Element {
            rsx! { JsonState { text: "{\"a\": 1}".to_string() } }
        }
        fn bad() -> Element {
            rsx! { JsonState { text: "{".to_string() } }
        }
        assert!(render(ok).contains("JSON valid"));
        let html = render(bad);
        assert!(html.contains("Not valid JSON"), "{html}");
        assert!(html.contains("alert"), "{html}");
    }

    #[test]
    fn the_gutter_numbers_every_line() {
        fn app() -> Element {
            rsx! { JsonEditor { text: "{\n  \"a\": 1\n}".to_string(), label: "Payload".to_string(), on_input: |_| {} } }
        }
        let html = render(app);
        assert!(html.contains("<div>3</div>"), "{html}");
        assert!(!html.contains("<div>4</div>"), "{html}");
    }
}
