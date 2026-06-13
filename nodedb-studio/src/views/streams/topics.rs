//! Streams · Durable topics: a table of replayable topics.

use dioxus::prelude::*;

#[component]
pub fn StreamsTopics() -> Element {
    rsx! {
        div { style: "overflow:auto;",
            div { class: "view-padded",
                div { class: "view-header",
                    div {
                        h1 { style: "font-size:16px;", "Durable topics" }
                        p { "Kafka-style replayable topics with offsets and consumer groups." }
                    }
                    div { class: "view-header-actions",
                        button { class: "btn primary", "+ Topic" }
                    }
                }
                div { class: "card", style: "padding: 0;",
                    table { class: "table",
                        thead { tr {
                            th { "Topic" } th { "Partitions" } th { "Messages" } th { "Retention" }
                            th { "Consumers" } th { "Max lag" } th {}
                        } }
                        tbody {
                            tr {
                                td {
                                    strong { "order_placed" }
                                    br {}
                                    span { style: "color:var(--text-tertiary);font-family:var(--font-mono);font-size:11px;", "created 2026-04-02" }
                                }
                                td { class: "mono", "8" }
                                td { class: "mono", "2.4M" }
                                td { class: "mono", "7d" }
                                td { span { class: "pill ok", "3 active" } }
                                td { class: "mono", "142ms" }
                                td { button { class: "btn small", "Inspect" } }
                            }
                            tr {
                                td { strong { "user_signup" } }
                                td { class: "mono", "4" }
                                td { class: "mono", "88,209" }
                                td { class: "mono", "30d" }
                                td { span { class: "pill ok", "2 active" } }
                                td { class: "mono", "22ms" }
                                td { button { class: "btn small", "Inspect" } }
                            }
                            tr {
                                td { strong { "payment_failed" } }
                                td { class: "mono", "2" }
                                td { class: "mono", "12,488" }
                                td { class: "mono", "90d" }
                                td { span { class: "pill warn", "1 active · 1 stalled" } }
                                td { class: "mono", "4.2s" }
                                td { button { class: "btn small", "Inspect" } }
                            }
                        }
                    }
                }
            }
        }
    }
}
