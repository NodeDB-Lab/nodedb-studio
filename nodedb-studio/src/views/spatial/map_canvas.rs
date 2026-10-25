//! The map canvas: the drawing, the pointer, and the overlays, over a
//! computed Web Mercator window.
//!
//! There is no basemap and no tile server. The map draws the geometry it was
//! given, so a blank area means nothing was read there, not that nothing is
//! there. Labels and the scale come from the same projection as the
//! features.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::geo_ops::Shape;
use crate::models::geo_proj::map;
use crate::models::spatial::{GeoSource, LayerPage, SpatialSetup};
use crate::state::spatial::{MapTool, SpatialUi};
use crate::views::spatial::map_draw::{graticule, layout};
use crate::views::spatial::map_input::{
    CLICK_PX, KEY_PAN_PX, ZOOM_BUTTON, ZOOM_WHEEL, add_measure_point, add_vertex, cancel_drawing,
    drag_rectangle, drop_double_click_vertices, finish_polygon, finish_rectangle, pan_px, pick_at,
    start_rectangle, step_selection, zoom_at, zoom_centre,
};
use crate::views::spatial::map_overlay::{Caption, ChipBar, Controls};
use crate::views::spatial::map_svg::MapSvg;

#[derive(Debug, Clone, Copy, PartialEq)]
enum DragKind {
    Pan,
    Rect,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Drag {
    kind: DragKind,
    /// The last pointer position in client pixels.
    client: (f64, f64),
    travelled: f64,
}

#[component]
pub fn MapCanvas(setup: SpatialSetup, source: GeoSource) -> Element {
    let ui = use_context::<SpatialUi>();
    let mut map_ui = ui.map;
    let mut hover = use_signal(|| None::<(f64, f64)>);
    let mut drag = use_signal(|| None::<Drag>);
    // The last size the element reported. A scene or a reset can clear
    // `measured` after the only report, so the effect below restores it.
    let mut seen = use_signal(|| None::<(f64, f64)>);
    use_effect(move || {
        let measured = map_ui.read().measured;
        let Some(sz) = *seen.peek() else { return };
        if !measured {
            let mut m = map_ui.write();
            m.size = sz;
            m.measured = true;
        }
    });

    let m = map_ui.read().clone();
    let (size, view) = (m.size, m.view);
    let pages: Vec<&LayerPage> = m
        .shown
        .as_ref()
        .map(|v| {
            v.layers
                .iter()
                .filter(|p| m.visible(&p.source.key()))
                .collect()
        })
        .unwrap_or_default();
    let loaded: usize = pages.iter().map(|p| p.features.len()).sum();
    let items = view
        .map(|v| layout(v, size, &pages, m.selected.as_ref()))
        .unwrap_or_default();
    let (meridians, parallels) = view.map(|v| graticule(v, size)).unwrap_or_default();
    // The selected feature's title and position, when the loaded result holds it.
    let label = m.selected.as_ref().and_then(|r| {
        let f = m.shown.as_ref()?.feature(r)?;
        let Shape::Point(lon, lat) = f.shape else {
            return None;
        };
        let (x, y) = map::to_screen(view?, lon, lat, size);
        Some((f.title.clone(), x, y))
    });
    let cursor = match m.tool {
        MapTool::Select | MapTool::Pan => "pan",
        MapTool::Rectangle | MapTool::Polygon | MapTool::Measure => "draw",
    };
    let error = m.error.clone();
    let (selected, result, draft, measure) = (
        m.selected.clone(),
        m.result.clone().filter(|_| m.visible("result")),
        m.draft.clone().filter(|_| m.visible("draft")),
        m.measure.clone(),
    );
    let key_source = source.source.clone();

    rsx! {
        div {
            class: "sp-map {cursor}",
            tabindex: "0",
            role: "application",
            "aria-label": "Map. Arrow keys pan. Plus and minus zoom. Page Down and Page Up select the next or previous feature. Enter finishes a polygon. Escape cancels a draft.",
            onresize: move |e| {
                if let Ok(sz) = e.get_border_box_size() {
                    let (w, h) = (sz.width.round(), sz.height.round());
                    if w > 1.0 && h > 1.0 {
                        seen.set(Some((w, h)));
                        let mut m = map_ui.write();
                        if m.size != (w, h) || !m.measured {
                            m.size = (w, h);
                            m.measured = true;
                        }
                    }
                }
            },
            onkeydown: move |e| {
                match e.key() {
                    Key::ArrowLeft => pan_px(map_ui, KEY_PAN_PX, 0.0),
                    Key::ArrowRight => pan_px(map_ui, -KEY_PAN_PX, 0.0),
                    Key::ArrowUp => pan_px(map_ui, 0.0, KEY_PAN_PX),
                    Key::ArrowDown => pan_px(map_ui, 0.0, -KEY_PAN_PX),
                    Key::PageDown => step_selection(map_ui, &key_source, 1),
                    Key::PageUp => step_selection(map_ui, &key_source, -1),
                    Key::Enter => finish_polygon(map_ui),
                    Key::Escape => cancel_drawing(map_ui),
                    Key::Character(c) if c == "+" || c == "=" => zoom_centre(map_ui, ZOOM_BUTTON),
                    Key::Character(c) if c == "-" => zoom_centre(map_ui, 1.0 / ZOOM_BUTTON),
                    _ => return,
                }
                e.prevent_default();
            },
            if let Some(view) = view {
                MapSvg {
                    view,
                    size,
                    meridians,
                    parallels,
                    items,
                    selected,
                    label,
                    result,
                    draft,
                    measure,
                    loaded,
                    hover,
                }
                div {
                    class: "sp-hit",
                    onmousedown: move |e| {
                        let (p, c) = (e.element_coordinates(), e.client_coordinates());
                        let kind = if map_ui.peek().tool == MapTool::Rectangle {
                            start_rectangle(map_ui, p.x, p.y);
                            DragKind::Rect
                        } else {
                            DragKind::Pan
                        };
                        drag.set(Some(Drag { kind, client: (c.x, c.y), travelled: 0.0 }));
                    },
                    onmousemove: move |e| {
                        let p = e.element_coordinates();
                        hover.set(Some((p.x, p.y)));
                        let Some(d) = *drag.peek() else { return };
                        let c = e.client_coordinates();
                        let (dx, dy) = (c.x - d.client.0, c.y - d.client.1);
                        match d.kind {
                            DragKind::Pan => pan_px(map_ui, dx, dy),
                            DragKind::Rect => drag_rectangle(map_ui, p.x, p.y),
                        }
                        drag.set(Some(Drag {
                            client: (c.x, c.y),
                            travelled: d.travelled + dx.abs() + dy.abs(),
                            ..d
                        }));
                    },
                    onmouseup: move |e| {
                        let Some(d) = *drag.peek() else { return };
                        drag.set(None);
                        let p = e.element_coordinates();
                        let click = d.travelled < CLICK_PX;
                        let tool = map_ui.peek().tool;
                        match (tool, d.kind) {
                            (MapTool::Rectangle, _) => finish_rectangle(map_ui),
                            (MapTool::Select, DragKind::Pan) if click => pick_at(map_ui, p.x, p.y),
                            (MapTool::Polygon, DragKind::Pan) if click => add_vertex(map_ui, p.x, p.y),
                            (MapTool::Measure, DragKind::Pan) if click => add_measure_point(map_ui, p.x, p.y),
                            _ => {}
                        }
                    },
                    ondoubleclick: move |_| {
                        if map_ui.peek().tool == MapTool::Polygon {
                            drop_double_click_vertices(map_ui);
                            finish_polygon(map_ui);
                        }
                    },
                    onmouseleave: move |_| {
                        hover.set(None);
                        if drag.peek().is_some_and(|d| d.kind == DragKind::Rect) {
                            finish_rectangle(map_ui);
                        }
                        drag.set(None);
                    },
                    onwheel: move |e| {
                        e.prevent_default();
                        let Some(at) = *hover.peek() else { return };
                        let factor = if e.delta().strip_units().y < 0.0 { ZOOM_WHEEL } else { 1.0 / ZOOM_WHEEL };
                        zoom_at(map_ui, factor, at);
                    },
                }
                ChipBar { source: source.clone() }
                if let Some(e) = error {
                    div { class: "callout err sp-error", role: "alert",
                        Ico { icon: Icon::CircleAlert, size: 16 }
                        span { "{e} The last result stays on the map." }
                    }
                }
                Controls { setup: setup.clone(), source: source.clone() }
            } else {
                div { class: "sp-state", role: "status", "Framing the map…" }
            }
        }
        Caption { source: source.clone(), hover }
    }
}
