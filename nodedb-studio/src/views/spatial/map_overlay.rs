//! What sits over the map canvas: the pointer readout, the draft and measure
//! chips, the zoom controls with the scale bar, Search this area, and the
//! caption with the counts. Each reads the map state it shows.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::format::thousands;
use crate::models::geo_ops::{Bbox, Shape, haversine_m, point_within};
use crate::models::geo_proj::{PlaneView, length_label, map, scale_bar};
use crate::models::spatial::{GeoSource, LayerPage, SpatialSetup};
use crate::models::spatial_analyze::distance_text;
use crate::models::spatial_text::trimmed;
use crate::routes::Route;
use crate::routes::tabs::SpatialTab;
use crate::state::spatial::{Draft, SpatialUi};
use crate::views::spatial::map::{RENDER_LIMIT, search_here};
use crate::views::spatial::map_input::{ZOOM_BUTTON, finish_polygon, zoom_centre};

/// What the pointer's position reads out, in longitude and latitude.
#[component]
pub fn Readout(hover: Signal<Option<(f64, f64)>>, view: PlaneView, size: (f64, f64)) -> Element {
    let Some((px, py)) = hover() else {
        return rsx! {
            span { class: "sp-readout idle", "Move the pointer over the map to read its position" }
        };
    };
    let (lon, lat) = map::from_screen(view, px, py, size);
    rsx! {
        span { class: "sp-readout mono",
            span { class: "k", "Longitude" }
            " {trimmed(lon, 5)}"
            span { class: "k sp", "Latitude" }
            " {trimmed(lat, 5)}"
            span { class: "k sp", "EPSG:4326" }
        }
    }
}

/// The segment from the last drawn point to the pointer.
#[component]
pub fn Preview(hover: Signal<Option<(f64, f64)>>, from: (f64, f64)) -> Element {
    let Some((px, py)) = hover() else {
        return rsx! {};
    };
    rsx! {
        line { class: "sp-rubber", x1: "{from.0}", y1: "{from.1}", x2: "{px}", y2: "{py}" }
    }
}

/// The draft and the measurement, with what each can do next.
#[component]
pub fn ChipBar(source: GeoSource) -> Element {
    let ui = use_context::<SpatialUi>();
    let nav = use_navigator();
    let mut map_ui = ui.map;
    let m = map_ui.read();
    let (draft, measure) = (m.draft.clone(), m.measure.clone());
    if draft.is_none() && measure.is_empty() {
        return rsx! {};
    }
    let loaded: Vec<&LayerPage> = m
        .shown
        .as_ref()
        .map(|v| {
            v.layers
                .iter()
                .filter(|p| m.visible(&p.source.key()))
                .collect()
        })
        .unwrap_or_default();
    let inside = draft.as_ref().and_then(Draft::ring).map(|ring| {
        loaded
            .iter()
            .flat_map(|p| p.features.iter())
            .filter(|f| matches!(f.shape, Shape::Point(x, y) if point_within((x, y), std::slice::from_ref(&ring))))
            .count()
    });
    let loaded_count: usize = loaded.iter().map(|p| p.features.len()).sum();
    let measured = (measure.len() == 2).then(|| {
        format!(
            "{} · geodesic estimate",
            distance_text(haversine_m(measure[0], measure[1]))
        )
    });
    drop(m);

    rsx! {
        div { class: "sp-chipbar",
            if let Some(d) = draft {
                span { class: "sp-chip",
                    "{d.label()}"
                    if let (Some(n), true) = (inside, d.done()) {
                        " · {n} of {thousands(loaded_count as u64)} loaded features inside"
                    }
                }
                if d.done() {
                    button {
                        class: "btn sm",
                        disabled: d.ring().is_none(),
                        title: "Start a Within analysis with this area. It does not run.",
                        onclick: {
                            let source = source.source.clone();
                            move |_| {
                                let Some(d) = map_ui.peek().draft.clone() else { return };
                                let Some(ring) = d.ring() else { return };
                                let rect = match d {
                                    Draft::Rectangle { from, to, .. } => Some(Bbox::new(
                                        from.0.min(to.0),
                                        from.1.min(to.1),
                                        from.0.max(to.0),
                                        from.1.max(to.1),
                                    )),
                                    Draft::Polygon { .. } => None,
                                };
                                ui.analyze_area(&source, ring, rect);
                                nav.push(Route::Spatial { tab: SpatialTab::Analyze });
                            }
                        },
                        "Analyze in this area"
                    }
                } else {
                    button {
                        class: "btn sm",
                        disabled: !matches!(&d, Draft::Polygon { points, .. } if points.len() >= 3),
                        onclick: move |_| finish_polygon(map_ui),
                        "Finish polygon"
                    }
                }
                button {
                    class: "btn sm",
                    title: "Remove the draft. Nothing was stored.",
                    onclick: move |_| map_ui.write().draft = None,
                    "Clear draft"
                }
            }
            if let Some(text) = measured {
                span { class: "sp-chip", "Measure {text}" }
            } else if measure.len() == 1 {
                span { class: "sp-chip", "Measure: click a second point" }
            }
            if !measure.is_empty() {
                button {
                    class: "btn sm",
                    onclick: move |_| map_ui.write().measure.clear(),
                    "Clear measure"
                }
            }
        }
    }
}

/// Zoom, Fit, north, the scale bar, and Search this area.
#[component]
pub fn Controls(setup: SpatialSetup, source: GeoSource) -> Element {
    let ui = use_context::<SpatialUi>();
    let mut map_ui = ui.map;
    let (view, size, reading) = {
        let m = map_ui.read();
        (m.view, m.size, m.pending.is_some())
    };
    // The scale comes from the renderer's own ground scale at the middle.
    let bar = view.and_then(|v| {
        let (_, mid_lat) = map::from_screen(v, size.0 / 2.0, size.1 / 2.0, size);
        scale_bar(map::metres_per_px(v, mid_lat), 140.0)
    });
    rsx! {
        div { class: "sp-controls",
            div { class: "sp-zoom",
                button {
                    class: "icon-btn sp-ctl",
                    title: "Zoom in",
                    "aria-label": "Zoom in",
                    onclick: move |_| zoom_centre(map_ui, ZOOM_BUTTON),
                    Ico { icon: Icon::Plus, size: 18 }
                }
                button {
                    class: "icon-btn sp-ctl",
                    title: "Zoom out",
                    "aria-label": "Zoom out",
                    onclick: move |_| zoom_centre(map_ui, 1.0 / ZOOM_BUTTON),
                    Ico { icon: Icon::Minus, size: 18 }
                }
                button {
                    class: "icon-btn sp-ctl",
                    title: "Fit the visible layers",
                    "aria-label": "Fit the visible layers",
                    onclick: {
                        let (setup, own) = (setup.clone(), source.extent);
                        move |_| {
                            let mut m = map_ui.write();
                            let hidden = m.hidden.clone();
                            m.frame = setup
                                .sources
                                .iter()
                                .filter(|s| !hidden.contains(&s.source.key()))
                                .filter_map(|s| s.extent)
                                .reduce(Bbox::union)
                                .or(own)
                                .map(|b| b.grown(0.12));
                        }
                    },
                    Ico { icon: Icon::Maximize, size: 18 }
                }
                span {
                    class: "sp-north",
                    title: "North is up. The map does not rotate.",
                    role: "img",
                    "aria-label": "North is up",
                    span { class: "arrow", "aria-hidden": "true" }
                    "N"
                }
            }
            if let Some((metres, px)) = bar {
                div { class: "sp-scale", style: "width:{px:.0}px",
                    span { class: "sp-scale-label", "{length_label(metres)}" }
                }
            }
        }
        button {
            class: "btn sp-search",
            disabled: reading,
            title: "Read the features in the current view. Panning never does it for you.",
            onclick: {
                let setup = setup.clone();
                move |_| {
                    let mut m = map_ui.write();
                    search_here(&mut m, &setup);
                }
            },
            Ico { icon: Icon::Search, size: 16 }
            if reading { "Searching…" } else { "Search this area" }
        }
    }
}

/// The caption: counts by their own scope, and the pointer readout.
#[component]
pub fn Caption(source: GeoSource, hover: Signal<Option<(f64, f64)>>) -> Element {
    let ui = use_context::<SpatialUi>();
    let map_ui = ui.map;
    let m = map_ui.read();
    let (view, size) = (m.view, m.size);
    let page = m.shown.as_ref().and_then(|v| v.page(&source.source));
    let in_view = match (view, page) {
        (Some(v), Some(p)) => {
            let b = map::bounds(v, size);
            p.features.iter().filter(|f| f.shape.touches(b)).count()
        }
        _ => 0,
    };
    let loaded = page.map_or(0, |p| p.features.len());
    let capped = page.is_some_and(LayerPage::capped);
    let matching = page.map_or(0, |p| p.matching);
    let reading = m.pending.is_some();
    let moved = m.moved;
    let has_page = page.is_some();
    drop(m);
    rsx! {
        div { class: "sp-caption",
            span { class: "sp-counts",
                if !has_page && reading {
                    "Reading features…"
                } else if !has_page {
                    "No features read yet"
                } else {
                    "{thousands(in_view as u64)} {source.source.collection} in view"
                    span { class: "sep", "·" }
                    "{thousands(loaded as u64)} loaded"
                    if let Some(t) = source.geometry_count {
                        " of {thousands(t)} in the source"
                    } else {
                        " of an unknown total"
                    }
                    if capped {
                        span { class: "sep", "·" }
                        "limited to {thousands(u64::from(RENDER_LIMIT))} of {thousands(matching)} in the box"
                    }
                }
                if moved {
                    span {
                        class: "badge amber sp-stale",
                        title: "The view moved after the last read.",
                        "View moved: search again"
                    }
                }
            }
            if let Some(view) = view {
                Readout { hover, view, size }
            }
        }
    }
}
