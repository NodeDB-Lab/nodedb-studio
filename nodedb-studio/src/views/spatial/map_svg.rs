//! The map's drawing: graticule, result area, draft, markers, clusters,
//! measurement, and the selected feature's label. Positions come from the
//! window the canvas was given, and nothing here reads state.

use dioxus::prelude::*;

use crate::models::geo_ops::Shape;
use crate::models::geo_proj::{PlaneView, map};
use crate::models::spatial::FeatureRef;
use crate::state::spatial::{Draft, ResultLayer};
use crate::views::spatial::map_draw::{Item, Tick, polygon_path, ring_path, text_width};
use crate::views::spatial::map_overlay::Preview;

fn cluster_radius(n: usize) -> f64 {
    (9.0 + (n as f64).sqrt() * 1.6).min(18.0)
}

#[component]
pub fn MapSvg(
    view: PlaneView,
    size: (f64, f64),
    meridians: Vec<Tick>,
    parallels: Vec<Tick>,
    items: Vec<Item>,
    selected: Option<FeatureRef>,
    /// The selected feature's title and screen position, when loaded.
    label: Option<(String, f64, f64)>,
    result: Option<ResultLayer>,
    draft: Option<Draft>,
    measure: Vec<(f64, f64)>,
    loaded: usize,
    hover: Signal<Option<(f64, f64)>>,
) -> Element {
    let screen = |p: (f64, f64)| map::to_screen(view, p.0, p.1, size);
    rsx! {
        svg {
            class: "sp-svg",
            view_box: "0 0 {size.0} {size.1}",
            preserve_aspect_ratio: "none",
            role: "img",
            "aria-label": "Map of {loaded} loaded features. The graticule is a display aid.",
            for t in meridians.iter() {
                g { key: "m{t.label}",
                    line { class: "sp-grid", x1: "{t.at}", y1: "0", x2: "{t.at}", y2: "{size.1}" }
                    text { class: "sp-tick", x: "{t.at + 5.0}", y: "16", "{t.label}" }
                }
            }
            for t in parallels.iter() {
                g { key: "p{t.label}",
                    line { class: "sp-grid", x1: "0", y1: "{t.at}", x2: "{size.0}", y2: "{t.at}" }
                    // The bottom-left corner holds the zoom controls.
                    if t.at < size.1 - 130.0 {
                        text { class: "sp-tick", x: "6", y: "{t.at - 5.0}", "{t.label}" }
                    }
                }
            }
            if let Some(r) = result.clone() {
                if let Shape::Polygon(rings) = &r.query {
                    path { class: "sp-result-area", fill_rule: "evenodd", d: "{polygon_path(view, size, rings)}" }
                }
            }
            if let Some(d) = draft.clone() {
                match d {
                    Draft::Rectangle { .. } => rsx! {
                        if let Some(ring) = d.ring() {
                            path { class: "sp-draft", d: "{ring_path(view, size, &ring)}" }
                        }
                    },
                    Draft::Polygon { points, done } => {
                        let pts: Vec<(f64, f64)> = points.iter().map(|p| screen(*p)).collect();
                        let line: String = pts
                            .iter()
                            .enumerate()
                            .map(|(i, (x, y))| format!("{}{x:.1} {y:.1}", if i == 0 { "M" } else { "L" }))
                            .collect();
                        let drawn = if done { format!("{line}Z") } else { line };
                        rsx! {
                            path { class: if done { "sp-draft" } else { "sp-draft open" }, d: "{drawn}" }
                            for (i , (x , y)) in pts.iter().enumerate() {
                                circle { key: "dv{i}", class: "sp-vertex", cx: "{x}", cy: "{y}", r: "3.5" }
                            }
                            if !done {
                                if let Some(last) = pts.last() {
                                    Preview { hover, from: *last }
                                }
                            }
                        }
                    }
                }
            }
            for item in items.iter() {
                match item {
                    Item::Point(mk) => rsx! {
                        g { key: "{mk.feature.source.key()}/{mk.feature.id}",
                            circle { class: "sp-pt", cx: "{mk.x}", cy: "{mk.y}", r: "5" }
                            if selected.as_ref() == Some(&mk.feature) {
                                circle { class: "sp-ring", cx: "{mk.x}", cy: "{mk.y}", r: "10" }
                            }
                        }
                    },
                    Item::Cluster { x, y, members } => rsx! {
                        g { key: "c{x:.0}/{y:.0}", class: "sp-cluster",
                            circle { cx: "{x}", cy: "{y}", r: "{cluster_radius(members.len())}" }
                            text { x: "{x}", y: "{y + 4.0}", text_anchor: "middle", "{members.len()}" }
                        }
                    },
                }
            }
            if let Some(r) = result {
                for (id , (lon , lat)) in r.rows.iter() {
                    {
                        let (x, y) = screen((*lon, *lat));
                        rsx! { circle { key: "r{id}", class: "sp-pt result", cx: "{x}", cy: "{y}", r: "5.5" } }
                    }
                }
            }
            if measure.len() == 2 {
                {
                    let (a, b) = (screen(measure[0]), screen(measure[1]));
                    rsx! {
                        line { class: "sp-measure", x1: "{a.0}", y1: "{a.1}", x2: "{b.0}", y2: "{b.1}" }
                        circle { class: "sp-vertex", cx: "{a.0}", cy: "{a.1}", r: "3.5" }
                        circle { class: "sp-vertex", cx: "{b.0}", cy: "{b.1}", r: "3.5" }
                    }
                }
            }
            if measure.len() == 1 {
                {
                    let a = screen(measure[0]);
                    rsx! {
                        circle { class: "sp-vertex", cx: "{a.0}", cy: "{a.1}", r: "3.5" }
                        Preview { hover, from: a }
                    }
                }
            }
            if let Some((title , x , y)) = label {
                {
                    let w = text_width(&title);
                    let flip = x + 24.0 + w > size.0 - 8.0;
                    let lx = if flip { x - 24.0 - w } else { x + 24.0 };
                    let ly = y - 30.0;
                    let ex = if flip { lx + w } else { lx };
                    rsx! {
                        line { class: "sp-leader", x1: "{x}", y1: "{y}", x2: "{ex}", y2: "{ly + 26.0}" }
                        rect { class: "sp-label-box", x: "{lx}", y: "{ly}", width: "{w}", height: "26", rx: "4" }
                        text { class: "sp-label", x: "{lx + 9.0}", y: "{ly + 18.0}", "{title}" }
                    }
                }
            }
        }
    }
}
