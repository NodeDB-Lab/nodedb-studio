//! Strict storage-mode viewer: a typed, schema'd data grid.

use dioxus::prelude::*;

#[component]
pub fn StrictViewer() -> Element {
    let rows = [
        (
            "442003",
            "u_44182",
            "129.40",
            "USD",
            "shipped",
            "2026-06-12 18:04:21",
        ),
        (
            "442002",
            "u_77103",
            "2,481.00",
            "EUR",
            "processing",
            "2026-06-12 17:51:08",
        ),
        (
            "442001",
            "u_12998",
            "48.20",
            "USD",
            "shipped",
            "2026-06-12 17:40:11",
        ),
        (
            "442000",
            "u_44182",
            "312.99",
            "USD",
            "cancelled",
            "2026-06-12 16:22:55",
        ),
        (
            "441999",
            "u_92210",
            "74.10",
            "GBP",
            "shipped",
            "2026-06-12 15:48:00",
        ),
        (
            "441998",
            "u_44182",
            "1,022.50",
            "USD",
            "shipped",
            "2026-06-12 15:18:34",
        ),
        (
            "441997",
            "u_31001",
            "9.99",
            "USD",
            "shipped",
            "2026-06-12 14:55:12",
        ),
    ];
    rsx! {
        div { class: "engine-viewer",
            div { class: "data-grid-wrap",
                table { class: "data-grid",
                    thead {
                        tr {
                            th { "id " span { style: "opacity:0.5", "int8" } }
                            th { "customer_id " span { style: "opacity:0.5", "int8" } }
                            th { "total " span { style: "opacity:0.5", "decimal" } }
                            th { "currency " span { style: "opacity:0.5", "char(3)" } }
                            th { "status " span { style: "opacity:0.5", "enum" } }
                            th { "placed_at " span { style: "opacity:0.5", "timestamptz" } }
                        }
                    }
                    tbody {
                        for r in rows {
                            tr {
                                td { "{r.0}" }
                                td { "{r.1}" }
                                td { "{r.2}" }
                                td { "{r.3}" }
                                td { "{r.4}" }
                                td { "{r.5}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
