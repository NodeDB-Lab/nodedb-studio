//! Streams · Materialized view inspector: a continuously-updating result grid.

use dioxus::prelude::*;

#[component]
pub fn StreamsMv() -> Element {
    // (recently-updated?, user_id, events_24h, last_seen, top_path)
    let rows = [
        (true, "u_44182", "2,148", "04:23:18", "/dashboard"),
        (false, "u_77103", "1,902", "04:23:17", "/pricing"),
        (false, "u_12998", "1,741", "04:23:16", "/docs"),
        (true, "u_31001", "1,602", "04:23:17", "/blog"),
        (false, "u_91002", "1,488", "04:23:11", "/dashboard"),
        (false, "u_22018", "1,401", "04:23:14", "/settings"),
        (false, "u_55501", "1,328", "04:23:09", "/dashboard"),
        (false, "u_92210", "1,202", "04:23:08", "/pricing"),
    ];
    rsx! {
        div { class: "live-tail",
            div { class: "tail-toolbar",
                strong { style: "font-size:13px;", "mv_top_users_24h" }
                span { class: "pill ok", span { class: "dot" } "live · p50 lag 12ms" }
                div { style: "margin-left:auto; display:flex; gap:6px;",
                    button { class: "btn small", "Definition" }
                    button { class: "btn small", "Refresh policy" }
                }
            }
            div { class: "results-body", style: "overflow:auto;",
                table { class: "data-grid",
                    thead { tr { th {} th { "user_id" } th { "events_24h" } th { "last_seen" } th { "top_path" } } }
                    tbody {
                        for r in rows {
                            tr { style: if r.0 { "background: rgba(59,109,17,0.04);" } else { "" },
                                td { if r.0 { span { class: "tail-pulse" } } }
                                td { "{r.1}" }
                                td { "{r.2}" }
                                td { "{r.3}" }
                                td { "{r.4}" }
                            }
                        }
                    }
                }
            }
            div { class: "tail-footer",
                span { class: "tail-pulse" }
                span { "2 rows updated · 8s ago" }
                span { "view size: 12,481 rows" }
                span { "storage: 4.8 MB" }
            }
        }
    }
}
