//! Node shapes and glyphs, drawn into the canvas SVG and the legend.
//!
//! Kind shows through shape, glyph, and label as well as color: users are
//! circles, posts and articles rounded squares, topics hexagons.

use dioxus::prelude::*;

use crate::models::graph::NodeKind;
use crate::models::graph_scene::NODE_R;

/// The glyph's 24-unit drawing, as SVG path data.
fn glyph(kind: NodeKind) -> &'static [&'static str] {
    match kind {
        NodeKind::User => &[
            "M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2",
            "M12 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8Z",
        ],
        NodeKind::Post | NodeKind::Article => &[
            "M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z",
            "M14 2v4a2 2 0 0 0 2 2h4",
            "M16 13H8M16 17H8M10 9H8",
        ],
        NodeKind::Topic => &["M4 9h16M4 15h16M10 3 8 21M16 3l-2 18"],
        NodeKind::Order => &[
            "M2 3h2l2.7 12.4a2 2 0 0 0 2 1.6h9.8a2 2 0 0 0 2-1.6L22 7H5.1",
            "M9 21.5a.5.5 0 1 0 0-1 .5.5 0 0 0 0 1ZM19 21.5a.5.5 0 1 0 0-1 .5.5 0 0 0 0 1Z",
        ],
        NodeKind::Product => &[
            "M11 21.7a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16V8a2 2 0 0 0-1-1.7l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.7Z",
            "M3.3 7 12 12l8.7-5M12 22V12",
        ],
    }
}

/// Hexagon corners around the origin, pointy side up, radius `r`.
fn hexagon(r: f32) -> String {
    (0..6)
        .map(|i| {
            let a = std::f32::consts::FRAC_PI_3 * i as f32 - std::f32::consts::FRAC_PI_2;
            format!("{:.1},{:.1}", r * a.cos(), r * a.sin())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The CSS class of a kind: its colors come from `graph.css`.
pub fn kind_class(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::User => "k-user",
        NodeKind::Post => "k-post",
        NodeKind::Topic => "k-topic",
        NodeKind::Order => "k-order",
        NodeKind::Product => "k-product",
        NodeKind::Article => "k-article",
    }
}

/// The badge tone of a kind, matching its color on the canvas.
pub fn kind_tone(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::User => "badge blue",
        NodeKind::Post => "badge green",
        NodeKind::Topic => "badge amber",
        NodeKind::Order => "badge teal",
        NodeKind::Product => "badge orange",
        NodeKind::Article => "badge violet",
    }
}

/// The shape and glyph of one node, centred on the origin of its parent.
/// `r` is half the shape's width.
#[component]
pub fn NodeShape(kind: NodeKind, r: f32) -> Element {
    let k = kind_class(kind);
    let scale = r * 1.05 / 24.0;
    let off = -12.0 * scale;
    rsx! {
        match kind {
            NodeKind::User | NodeKind::Order | NodeKind::Product => rsx! {
                circle { class: "g-shape {k}", r: "{r}" }
            },
            NodeKind::Post | NodeKind::Article => rsx! {
                rect {
                    class: "g-shape {k}",
                    x: "{-r}",
                    y: "{-r}",
                    width: "{r * 2.0}",
                    height: "{r * 2.0}",
                    rx: "{r * 0.3}",
                }
            },
            NodeKind::Topic => rsx! {
                polygon { class: "g-shape {k}", points: "{hexagon(r + 1.0)}" }
            },
        }
        g {
            class: "g-glyph {k}",
            transform: "translate({off} {off}) scale({scale})",
            for (i, d) in glyph(kind).iter().enumerate() {
                path { key: "{i}", d: "{d}" }
            }
        }
    }
}

/// A kind's shape at legend size, in its own small SVG.
#[component]
pub fn LegendShape(kind: NodeKind) -> Element {
    let r = 9.0;
    rsx! {
        svg {
            class: "legend-shape",
            width: "20",
            height: "20",
            view_box: "-11 -11 22 22",
            "aria-hidden": "true",
            NodeShape { kind, r }
        }
    }
}

/// Half the width of a node on the canvas.
pub const R: f32 = NODE_R;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_a_glyph_and_a_class() {
        for k in NodeKind::ALL {
            assert!(!glyph(k).is_empty());
            assert!(kind_class(k).starts_with("k-"));
        }
    }

    #[test]
    fn a_hexagon_has_six_corners() {
        assert_eq!(hexagon(10.0).split(' ').count(), 6);
    }
}
