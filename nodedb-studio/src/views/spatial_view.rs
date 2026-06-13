//! Spatial map: layers sidebar + a placeholder map sketch (no real map widget).

use dioxus::prelude::*;

#[component]
pub fn SpatialView() -> Element {
    // (layer, count, checked)
    let layers = [
        ("store_locations", "2.1k", true),
        ("delivery_zones", "48", true),
        ("last_24h_orders", "12k", false),
        ("warehouses", "14", false),
    ];
    rsx! {
        div { class: "view active",
            div { style: "height: 100%; display: grid; grid-template-columns: 260px 1fr;",
                div { style: "background: var(--bg-secondary); border-right: 0.5px solid var(--border-mid); overflow:auto; padding: 14px;",
                    div { class: "eyebrow", style: "margin-bottom: 8px;", "Layers" }
                    for (layer, count, checked) in layers {
                        label { style: "display:flex; align-items:center; gap:8px; padding:6px 4px; font-size:12px;",
                            input { r#type: "checkbox", checked }
                            span { "{layer}" }
                            span { style: "margin-left:auto; font-family:var(--font-mono); font-size:10px; color:var(--text-tertiary);", "{count}" }
                        }
                    }
                    div { class: "eyebrow", style: "margin: 16px 0 8px;", "Selected feature" }
                    div { class: "card", style: "padding:12px;",
                        div { style: "font-weight:500; font-size:13px;", "Store #482" }
                        div { style: "font-family:var(--font-mono); font-size:11px; color:var(--text-tertiary); margin-bottom:8px;", "Kuala Lumpur, MY" }
                        div { style: "display:grid; grid-template-columns: auto 1fr; gap:4px 12px; font-family:var(--font-mono); font-size:11px;",
                            span { style: "color:var(--text-tertiary)", "type" } span { "flagship" }
                            span { style: "color:var(--text-tertiary)", "opens" } span { "2024-08-12" }
                            span { style: "color:var(--text-tertiary)", "orders 24h" } span { "184" }
                            span { style: "color:var(--text-tertiary)", "lat,lng" } span { "3.158, 101.713" }
                        }
                    }
                    div { class: "eyebrow", style: "margin: 16px 0 8px;", "Query" }
                    pre { style: "background:var(--bg-primary); border:0.5px solid var(--border-mid); border-radius:5px; padding:8px; margin:0; font-family:var(--font-mono); font-size:10px; line-height:1.6;",
                        "SELECT *\nFROM store_locations\nWHERE ST_Within(\n  geom,\n  ST_MakeEnvelope(...)\n)"
                    }
                }
                div { class: "spatial-map", style: "height:100%;",
                    svg { view_box: "0 0 1000 600", style: "width:100%; height:100%;",
                        path { d: "M50 280 Q 150 240 250 260 T 450 240 T 650 270 T 850 250 L 950 260 L 950 380 Q 850 400 700 390 T 500 400 T 300 380 T 50 400 Z", fill: "rgba(24,95,165,0.08)", stroke: "rgba(24,95,165,0.4)", stroke_width: "0.7" }
                        path { d: "M280 250 L 380 240 L 420 280 L 380 340 L 300 350 L 250 310 Z", fill: "rgba(59,109,17,0.10)", stroke: "rgba(59,109,17,0.5)", stroke_width: "1" }
                        path { d: "M520 240 L 620 230 L 660 270 L 620 330 L 540 340 L 490 300 Z", fill: "rgba(59,109,17,0.10)", stroke: "rgba(59,109,17,0.5)", stroke_width: "1" }
                        circle { cx: "180", cy: "290", r: "4", fill: "#1a1a18" }
                        circle { cx: "240", cy: "270", r: "4", fill: "#1a1a18" }
                        circle { cx: "310", cy: "295", r: "4", fill: "#1a1a18" }
                        circle { cx: "340", cy: "280", r: "6", fill: "#9c2424", stroke: "var(--bg-primary)", stroke_width: "2" }
                        circle { cx: "420", cy: "280", r: "4", fill: "#1a1a18" }
                        circle { cx: "510", cy: "300", r: "4", fill: "#1a1a18" }
                        circle { cx: "580", cy: "280", r: "4", fill: "#1a1a18" }
                        circle { cx: "650", cy: "300", r: "4", fill: "#1a1a18" }
                        circle { cx: "720", cy: "290", r: "4", fill: "#1a1a18" }
                        circle { cx: "800", cy: "280", r: "4", fill: "#1a1a18" }
                        circle { cx: "860", cy: "300", r: "4", fill: "#1a1a18" }
                    }
                    div { style: "position:absolute; top:14px; left:14px; padding:8px 12px; background:var(--bg-primary); border:0.5px solid var(--border-mid); border-radius:6px; font-family:var(--font-mono); font-size:11px;",
                        "ST_Within · 2,108 features visible · zoom 6"
                    }
                    div { style: "position:absolute; bottom:14px; right:14px; display:flex; flex-direction:column; background:var(--bg-primary); border:0.5px solid var(--border-mid); border-radius:6px; overflow:hidden;",
                        button { class: "btn ghost small", style: "border-radius:0; border-bottom:0.5px solid var(--border-soft);", "+" }
                        button { class: "btn ghost small", style: "border-radius:0;", "−" }
                    }
                }
            }
        }
    }
}
