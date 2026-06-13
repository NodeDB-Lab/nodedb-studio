//! Document storage-mode viewer: a JSON document tree.

use dioxus::prelude::*;

#[component]
pub fn DocumentViewer() -> Element {
    rsx! {
        div { class: "engine-viewer active",
            div { class: "json-tree",
                div { class: "json-row", span { class: "json-punct", "{{" } }
                div { class: "json-row",
                    span { class: "indent", "  " }
                    span { class: "json-key", "\"_id\"" }
                    span { class: "json-punct", ": " }
                    span { class: "json-string", "\"evt_01H8QXG2K…\"" }
                    span { class: "json-punct", "," }
                }
                div { class: "json-row",
                    span { class: "indent", "  " }
                    span { class: "json-key", "\"type\"" }
                    span { class: "json-punct", ": " }
                    span { class: "json-string", "\"page_view\"" }
                    span { class: "json-punct", "," }
                }
                div { class: "json-row",
                    span { class: "indent", "  " }
                    span { class: "json-key", "\"user_id\"" }
                    span { class: "json-punct", ": " }
                    span { class: "json-string", "\"u_44182\"" }
                    span { class: "json-punct", "," }
                }
                div { class: "json-row",
                    span { class: "indent", "  " }
                    span { class: "json-key", "\"ts\"" }
                    span { class: "json-punct", ": " }
                    span { class: "json-string", "\"2026-06-13T04:22:18Z\"" }
                    span { class: "json-punct", "," }
                }
                div { class: "json-row",
                    span { class: "indent", "  " }
                    span { class: "json-key", "\"props\"" }
                    span { class: "json-punct", ": {{" }
                }
                div { class: "json-row",
                    span { class: "indent", "    " }
                    span { class: "json-key", "\"path\"" }
                    span { class: "json-punct", ": " }
                    span { class: "json-string", "\"/dashboard\"" }
                    span { class: "json-punct", "," }
                }
                div { class: "json-row",
                    span { class: "indent", "    " }
                    span { class: "json-key", "\"referrer\"" }
                    span { class: "json-punct", ": " }
                    span { class: "json-string", "\"google\"" }
                    span { class: "json-punct", "," }
                }
                div { class: "json-row",
                    span { class: "indent", "    " }
                    span { class: "json-key", "\"ms_to_load\"" }
                    span { class: "json-punct", ": " }
                    span { class: "json-num", "348" }
                }
                div { class: "json-row",
                    span { class: "indent", "  " }
                    span { class: "json-punct", "}}," }
                }
                div { class: "json-row",
                    span { class: "indent", "  " }
                    span { class: "json-key", "\"tags\"" }
                    span { class: "json-punct", ": [" }
                    span { class: "json-string", "\"web\"" }
                    span { class: "json-punct", ", " }
                    span { class: "json-string", "\"mobile\"" }
                    span { class: "json-punct", "]" }
                }
                div { class: "json-row", span { class: "json-punct", "}}" } }
                div { class: "json-row", style: "margin-top:14px; color: var(--text-tertiary);",
                    "— showing document 1 of 2,400,182. Use Query to browse a result set."
                }
            }
        }
    }
}
