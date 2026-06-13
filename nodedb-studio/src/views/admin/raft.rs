//! Admin · Raft group detail: leader/term stats + log replication table.

use dioxus::prelude::*;

#[component]
pub fn RaftView() -> Element {
    // (node, leader?, match_index, lag, lag_warn, heartbeat, state, state_pill)
    let rows = [
        ("node-1", true, "1,488,201", "—", false, "—", "—", ""),
        (
            "node-2",
            false,
            "1,488,201",
            "0",
            false,
            "12 ms",
            "replicating",
            "",
        ),
        (
            "node-3",
            false,
            "1,488,198",
            "3",
            false,
            "14 ms",
            "replicating",
            "",
        ),
        (
            "node-4",
            false,
            "1,488,200",
            "1",
            false,
            "15 ms",
            "replicating",
            "",
        ),
        (
            "node-5",
            false,
            "1,486,892",
            "1,309",
            true,
            "88 ms",
            "catching up",
            "warn",
        ),
        (
            "node-6",
            false,
            "1,488,201",
            "0",
            false,
            "11 ms",
            "replicating",
            "",
        ),
        (
            "node-7",
            false,
            "1,488,199",
            "2",
            false,
            "13 ms",
            "replicating",
            "",
        ),
    ];
    rsx! {
        div { class: "subview active",
            div { class: "view-header",
                div { h1 { style: "font-size:16px;", "Raft group · raft-group-1" } p { "analytics · 7 nodes · term 142" } }
            }
            div { class: "placeholder-grid",
                div { class: "ph-stat", div { class: "l", "LEADER" } div { class: "v", style: "font-size:16px;", "node-1" } div { class: "sub", "elected 4h 22m ago" } }
                div { class: "ph-stat", div { class: "l", "TERM" } div { class: "v", "142" } div { class: "sub", "stable" } }
                div { class: "ph-stat", div { class: "l", "COMMIT INDEX" } div { class: "v", style: "font-size:16px;", "1,488,201" } div { class: "sub", "+820/s" } }
                div { class: "ph-stat", div { class: "l", "MAX LAG" } div { class: "v", style: "color:var(--text-warning)", "142 ms" } div { class: "sub", "node-5" } }
            }
            div { class: "ph-section",
                h2 { "Log replication" }
                div { class: "card", style: "padding:0;",
                    table { class: "table",
                        thead { tr { th { "Node" } th { "Match index" } th { "Lag" } th { "Heartbeat" } th { "State" } } }
                        tbody {
                            for r in rows {
                                tr {
                                    td {
                                        "{r.0} "
                                        if r.1 { span { class: "pill info", "leader" } }
                                    }
                                    td { class: "mono", "{r.2}" }
                                    td { class: "mono", style: if r.4 { "color:var(--text-warning)" } else { "" }, "{r.3}" }
                                    td { class: "mono", "{r.5}" }
                                    td {
                                        if r.7 == "warn" {
                                            span { class: "pill warn", "{r.6}" }
                                        } else {
                                            "{r.6}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
