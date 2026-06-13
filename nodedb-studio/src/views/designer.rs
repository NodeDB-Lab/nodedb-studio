//! Schema designer: a static ER-style canvas of collections and relationships.
//! Placeholder SVG (no real diagramming library).

use dioxus::prelude::*;

#[component]
pub fn Designer() -> Element {
    rsx! {
        div { class: "view active",
            div { style: "height: 100%; display: grid; grid-template-rows: auto 1fr;",
                div { style: "padding: 10px 16px; background: var(--bg-secondary); border-bottom: 0.5px solid var(--border-mid); display: flex; align-items: center; gap: 10px;",
                    button { class: "btn small", "+ Collection" }
                    button { class: "btn small", "+ Relationship" }
                    div { style: "margin-left: auto; display: flex; gap: 6px;",
                        button { class: "btn small ghost", "Auto-layout" }
                        button { class: "btn small ghost", "Export DDL" }
                        button { class: "btn small primary", "Apply changes (3 pending)" }
                    }
                }
                div { class: "designer-canvas",
                    svg { view_box: "0 0 1000 600", style: "width:100%; height:100%;",
                        path { d: "M180,180 L380,180", stroke: "var(--text-tertiary)", stroke_width: "1", fill: "none" }
                        path { d: "M280,260 L280,360", stroke: "var(--text-tertiary)", stroke_width: "1", fill: "none" }
                        path { d: "M480,260 L660,360", stroke: "var(--text-tertiary)", stroke_width: "1", fill: "none" }
                        path { d: "M180,460 L380,460", stroke: "var(--text-tertiary)", stroke_width: "1", fill: "none" }

                        g { transform: "translate(60, 100)",
                            rect { class: "designer-table", width: "220", height: "160", rx: "6" }
                            text { x: "14", y: "22", font_family: "var(--font-sans)", font_size: "12", font_weight: "600", fill: "var(--text-primary)", "users" }
                            text { x: "200", y: "22", font_family: "var(--font-mono)", font_size: "9", fill: "var(--text-tertiary)", text_anchor: "end", "DOCUMENT" }
                            line { x1: "0", y1: "34", x2: "220", y2: "34", stroke: "var(--border-mid)", stroke_width: "0.5" }
                            text { x: "14", y: "54", font_family: "var(--font-mono)", font_size: "11", fill: "var(--text-primary)", "_id" tspan { fill: "var(--text-tertiary)", "  string" } }
                            text { x: "14", y: "72", font_family: "var(--font-mono)", font_size: "11", fill: "var(--text-primary)", "name" tspan { fill: "var(--text-tertiary)", "  string" } }
                            text { x: "14", y: "90", font_family: "var(--font-mono)", font_size: "11", fill: "var(--text-primary)", "email" tspan { fill: "var(--text-tertiary)", "  string · unique" } }
                            text { x: "14", y: "108", font_family: "var(--font-mono)", font_size: "11", fill: "var(--text-primary)", "country" tspan { fill: "var(--text-tertiary)", "  char(2)" } }
                            text { x: "14", y: "126", font_family: "var(--font-mono)", font_size: "11", fill: "var(--text-primary)", "created_at" tspan { fill: "var(--text-tertiary)", "  timestamp" } }
                        }
                        g { transform: "translate(380, 100)",
                            rect { class: "designer-table", width: "220", height: "160", rx: "6" }
                            text { x: "14", y: "22", font_family: "var(--font-sans)", font_size: "12", font_weight: "600", fill: "var(--text-primary)", "orders" }
                            text { x: "200", y: "22", font_family: "var(--font-mono)", font_size: "9", fill: "var(--text-tertiary)", text_anchor: "end", "STRICT" }
                            line { x1: "0", y1: "34", x2: "220", y2: "34", stroke: "var(--border-mid)", stroke_width: "0.5" }
                            text { x: "14", y: "54", font_family: "var(--font-mono)", font_size: "11", "id" tspan { fill: "var(--text-tertiary)", "  int8 · pk" } }
                            text { x: "14", y: "72", font_family: "var(--font-mono)", font_size: "11", "user_id" tspan { fill: "var(--text-tertiary)", "  string · fk" } }
                            text { x: "14", y: "90", font_family: "var(--font-mono)", font_size: "11", "total" tspan { fill: "var(--text-tertiary)", "  decimal(10,2)" } }
                            text { x: "14", y: "108", font_family: "var(--font-mono)", font_size: "11", "status" tspan { fill: "var(--text-tertiary)", "  enum" } }
                            text { x: "14", y: "126", font_family: "var(--font-mono)", font_size: "11", "placed_at" tspan { fill: "var(--text-tertiary)", "  timestamptz" } }
                        }
                        g { transform: "translate(180, 360)",
                            rect { class: "designer-table", width: "200", height: "140", rx: "6" }
                            text { x: "14", y: "22", font_family: "var(--font-sans)", font_size: "12", font_weight: "600", "events" }
                            text { x: "180", y: "22", font_family: "var(--font-mono)", font_size: "9", fill: "var(--text-tertiary)", text_anchor: "end", "DOCUMENT" }
                            line { x1: "0", y1: "34", x2: "200", y2: "34", stroke: "var(--border-mid)", stroke_width: "0.5" }
                            text { x: "14", y: "54", font_family: "var(--font-mono)", font_size: "11", "_id" tspan { fill: "var(--text-tertiary)", "  string" } }
                            text { x: "14", y: "72", font_family: "var(--font-mono)", font_size: "11", "user_id" tspan { fill: "var(--text-tertiary)", "  string" } }
                            text { x: "14", y: "90", font_family: "var(--font-mono)", font_size: "11", "type" tspan { fill: "var(--text-tertiary)", "  string" } }
                            text { x: "14", y: "108", font_family: "var(--font-mono)", font_size: "11", "ts" tspan { fill: "var(--text-tertiary)", "  timestamp" } }
                        }
                        g { transform: "translate(660, 360)",
                            rect { class: "designer-table", width: "200", height: "140", rx: "6" }
                            text { x: "14", y: "22", font_family: "var(--font-sans)", font_size: "12", font_weight: "600", "products" }
                            text { x: "180", y: "22", font_family: "var(--font-mono)", font_size: "9", fill: "var(--text-tertiary)", text_anchor: "end", "STRICT" }
                            line { x1: "0", y1: "34", x2: "200", y2: "34", stroke: "var(--border-mid)", stroke_width: "0.5" }
                            text { x: "14", y: "54", font_family: "var(--font-mono)", font_size: "11", "sku" tspan { fill: "var(--text-tertiary)", "  string · pk" } }
                            text { x: "14", y: "72", font_family: "var(--font-mono)", font_size: "11", "name" tspan { fill: "var(--text-tertiary)", "  string" } }
                            text { x: "14", y: "90", font_family: "var(--font-mono)", font_size: "11", "price" tspan { fill: "var(--text-tertiary)", "  decimal(10,2)" } }
                        }
                        text { x: "280", y: "172", font_family: "var(--font-mono)", font_size: "9", fill: "var(--text-tertiary)", text_anchor: "middle", "PLACED · 1:N" }
                        text { x: "180", y: "452", font_family: "var(--font-mono)", font_size: "9", fill: "var(--text-tertiary)", text_anchor: "middle", "EMITTED" }
                    }
                }
            }
        }
    }
}
