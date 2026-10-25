//! A read-only JSON block: line numbers and syntax colors.

use dioxus::prelude::*;

use crate::models::record_json::tokens;

#[component]
pub fn JsonView(text: String, #[props(default)] class: String) -> Element {
    let lines: Vec<&str> = text.lines().collect();
    rsx! {
        div { class: "code-editor json-view {class}", role: "region", "aria-label": "JSON",
            div { class: "gutter", "aria-hidden": "true",
                for n in 1..=lines.len() {
                    div { key: "{n}", "{n}" }
                }
            }
            div { class: "code",
                for (i, line) in lines.iter().enumerate() {
                    div { key: "{i}", class: "code-line",
                        for (j, (kind, part)) in tokens(line).into_iter().enumerate() {
                            span { key: "{j}", class: "{kind.class()}", "{part}" }
                        }
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
    fn lines_are_numbered_and_colored() {
        fn app() -> Element {
            rsx! { JsonView { text: "{\n  \"a\": 1\n}".to_string() } }
        }
        let html = render(app);
        assert!(html.contains("tok-key"), "{html}");
        assert!(html.contains("tok-num"), "{html}");
        assert_eq!(html.matches("code-line").count(), 3, "{html}");
    }
}
