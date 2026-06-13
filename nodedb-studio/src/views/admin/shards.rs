//! Admin · Shard map: placement and replication factor across nodes.

use dioxus::prelude::*;

#[component]
pub fn ShardsView() -> Element {
    // (shard, range, leader, replicas, size, pill-css, status)
    let rows = [
        (
            "s_001",
            "[0000, 1FFF]",
            "node-1",
            "node-2, node-3",
            "2.4 GB",
            "ok",
            "healthy",
        ),
        (
            "s_002",
            "[2000, 3FFF]",
            "node-2",
            "node-3, node-4",
            "2.1 GB",
            "ok",
            "healthy",
        ),
        (
            "s_003",
            "[4000, 5FFF]",
            "node-3",
            "node-4, node-5",
            "2.8 GB",
            "warn",
            "replica lag",
        ),
        (
            "s_004",
            "[6000, 7FFF]",
            "node-4",
            "node-5, node-6",
            "1.9 GB",
            "ok",
            "healthy",
        ),
        (
            "s_005",
            "[8000, 9FFF]",
            "node-5",
            "node-6, node-7",
            "2.2 GB",
            "ok",
            "healthy",
        ),
        (
            "s_006",
            "[A000, BFFF]",
            "node-6",
            "node-7, node-1",
            "2.0 GB",
            "ok",
            "healthy",
        ),
        (
            "s_007",
            "[C000, DFFF]",
            "node-7",
            "node-1, node-2",
            "2.3 GB",
            "ok",
            "healthy",
        ),
        (
            "s_008",
            "[E000, FFFF]",
            "node-1",
            "node-2, node-3",
            "2.5 GB",
            "ok",
            "healthy",
        ),
    ];
    rsx! {
        div { class: "subview active",
            div { class: "view-header",
                div { h1 { style: "font-size:16px;", "Shard map" } p { "Placement and replication factor across nodes." } }
                div { class: "view-header-actions", button { class: "btn", "Rebalance" } }
            }
            div { class: "card", style: "padding:0;",
                table { class: "table",
                    thead { tr { th { "Shard" } th { "Range" } th { "Leader" } th { "Replicas" } th { "Size" } th { "Status" } } }
                    tbody {
                        for r in rows {
                            tr {
                                td { class: "mono", "{r.0}" }
                                td { class: "mono", "{r.1}" }
                                td { "{r.2}" }
                                td { "{r.3}" }
                                td { class: "mono", "{r.4}" }
                                td { span { class: "pill {r.5}", "{r.6}" } }
                            }
                        }
                    }
                }
            }
        }
    }
}
