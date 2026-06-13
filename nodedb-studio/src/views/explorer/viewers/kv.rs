//! KV storage-mode viewer: a key/value grid.

use dioxus::prelude::*;

#[component]
pub fn KvViewer() -> Element {
    let rows = [
        ("flag:graphrag_v2", "bool", "5 B", "—", "true"),
        ("flag:new_explorer_skin", "bool", "5 B", "—", "false"),
        (
            "session:u_44182",
            "json",
            "2.1 KB",
            "14m",
            "{{ \"user\": \"u_44182\", \"since\": \"…\", … }}",
        ),
        ("rate:/api/search", "counter", "8 B", "1m", "491"),
        ("cache:home_dashboard", "bytes", "18 KB", "2h", "(binary)"),
    ];
    rsx! {
        div { class: "engine-viewer",
            div { class: "data-grid-wrap",
                table { class: "data-grid",
                    thead { tr { th { "key" } th { "type" } th { "size" } th { "ttl" } th { "preview" } } }
                    tbody {
                        for r in rows {
                            tr {
                                td { "{r.0}" }
                                td { "{r.1}" }
                                td { "{r.2}" }
                                td { "{r.3}" }
                                td { "{r.4}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
