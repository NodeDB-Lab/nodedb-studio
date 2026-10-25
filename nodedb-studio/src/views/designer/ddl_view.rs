//! DDL as lines of code. A diff marks each added line.

use dioxus::prelude::*;

#[component]
pub fn Ddl(lines: Vec<String>, #[props(default = false)] diff: bool) -> Element {
    rsx! {
        pre { class: if diff { "ddl diff" } else { "ddl" }, tabindex: "0",
            for (n , line) in lines.iter().enumerate() {
                span {
                    key: "{n}",
                    class: if line.starts_with("--") { "ddl-line comment" } else { "ddl-line" },
                    if diff {
                        span { class: "ddl-mark", "aria-hidden": "true",
                            if line.is_empty() { "" } else { "+" }
                        }
                    }
                    "{line}"
                }
            }
        }
    }
}
