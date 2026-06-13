//! Graph Explorer · Builder mode: a visual pattern builder + canvas.

use dioxus::prelude::*;

#[component]
pub fn GxBuilder() -> Element {
    rsx! {
        div { class: "gx-body",
            div { style: "padding: 16px 20px; background: var(--bg-primary); border-bottom: 0.5px solid var(--border-mid);",
                div { style: "display: flex; gap: 8px; align-items: center; flex-wrap: wrap;",
                    span { class: "pill info", "User" }
                    span { style: "color: var(--text-tertiary); font-family: var(--font-mono);", "─[POSTED]→" }
                    span { class: "pill ok", "Post" }
                    span { style: "color: var(--text-tertiary); font-family: var(--font-mono);", "─[TAGGED]→" }
                    span { class: "pill warn", "Topic" }
                    button { class: "btn small ghost", "+ Hop" }
                }
                div { style: "margin-top: 14px; display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 12px;",
                    div { class: "form-field", label { "User.country" } select { option { "= MY" } option { "any" } } }
                    div { class: "form-field", label { "Post.created_at" } select { option { "last 30 days" } } }
                    div { class: "form-field", label { "Topic.name" } select { option { "= databases" } } }
                }
            }
            div { class: "gx-canvas",
                svg { view_box: "0 0 1000 500", style: "width:100%; height:100%;",
                    path { class: "gx-edge", d: "M200,250 L 500,250" }
                    path { class: "gx-edge", d: "M500,250 L 800,250" }
                    g { class: "gx-node", circle { cx: "200", cy: "250", r: "40", fill: "#185fa5" } text { x: "200", y: "254", fill: "white", "User" } }
                    g { class: "gx-node", circle { cx: "500", cy: "250", r: "40", fill: "#3b6d11" } text { x: "500", y: "254", fill: "white", "Post" } }
                    g { class: "gx-node", circle { cx: "800", cy: "250", r: "40", fill: "#854f0b" } text { x: "800", y: "254", fill: "white", "Topic" } }
                    text { x: "350", y: "240", fill: "var(--text-tertiary)", font_family: "var(--font-mono)", font_size: "11", "POSTED" }
                    text { x: "650", y: "240", fill: "var(--text-tertiary)", font_family: "var(--font-mono)", font_size: "11", "TAGGED" }
                }
            }
        }
    }
}
