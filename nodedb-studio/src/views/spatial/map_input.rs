//! What the map's pointer and keys do to the map state. Each function is one
//! gesture, so the canvas only routes events to them.

use dioxus::prelude::*;

use crate::models::geo_ops::Shape;
use crate::models::geo_proj::map;
use crate::models::spatial::{FeatureRef, LayerPage, SourceRef};
use crate::state::spatial::{Draft, MapUi};
use crate::views::spatial::map_draw::{Item, SPLIT_RES, layout, pick};

/// Pointer travel, in pixels, under which a press is a click.
pub const CLICK_PX: f64 = 4.0;
pub const ZOOM_BUTTON: f64 = 1.5;
pub const ZOOM_WHEEL: f64 = 1.25;
pub const KEY_PAN_PX: f64 = 80.0;
const ZOOM_CLUSTER: f64 = 2.2;

/// Zoom about the middle of the map.
pub fn zoom_centre(map_ui: Signal<MapUi>, factor: f64) {
    let size = map_ui.peek().size;
    zoom_at(map_ui, factor, (size.0 / 2.0, size.1 / 2.0));
}

/// Zoom about a screen point. The view moved, so the result is stale.
pub fn zoom_at(mut map_ui: Signal<MapUi>, factor: f64, at: (f64, f64)) {
    let mut m = map_ui.write();
    let size = m.size;
    if let Some(v) = m.view {
        m.view = Some(v.zoomed_about(factor, at, size, map::limits()));
        m.moved = true;
    }
}

/// Move the content by a drag of `(dx, dy)` pixels.
pub fn pan_px(mut map_ui: Signal<MapUi>, dx: f64, dy: f64) {
    let mut m = map_ui.write();
    if let Some(v) = m.view {
        m.view = Some(v.panned_px(dx, dy));
        m.moved = true;
    }
}

/// The pointer clicked at `(px, py)`: a marker selects, a cluster zooms in
/// or, at the finest scale, selects its first member, and empty map clears
/// the selection.
pub fn pick_at(mut map_ui: Signal<MapUi>, px: f64, py: f64) {
    let (view, size, selected, shown, hidden) = {
        let m = map_ui.peek();
        (
            m.view,
            m.size,
            m.selected.clone(),
            m.shown.clone(),
            m.hidden.clone(),
        )
    };
    let (Some(view), Some(shown)) = (view, shown) else {
        return;
    };
    let pages: Vec<&LayerPage> = shown
        .layers
        .iter()
        .filter(|p| !hidden.contains(&p.source.key()))
        .collect();
    let items = layout(view, size, &pages, selected.as_ref());
    let Some(hit) = pick(&items, px, py) else {
        map_ui.write().select(None);
        return;
    };
    match hit {
        Item::Point(mk) => map_ui.write().select(Some(mk.feature.clone())),
        Item::Cluster { x, y, members } => {
            if view.res <= SPLIT_RES {
                let first = members.first().map(|mk| mk.feature.clone());
                map_ui.write().select(first);
            } else {
                zoom_at(map_ui, ZOOM_CLUSTER, (*x, *y));
            }
        }
    }
}

/// Move the selection along the loaded features of `source`, and bring the
/// selected one into view.
pub fn step_selection(mut map_ui: Signal<MapUi>, source: &SourceRef, delta: i64) {
    let mut m = map_ui.write();
    let Some(shown) = m.shown.clone() else { return };
    let Some(page) = shown.page(source) else {
        return;
    };
    if page.features.is_empty() {
        return;
    }
    let at = m.selected.as_ref().and_then(|r| {
        page.features
            .iter()
            .position(|f| f.id == r.id && r.source == *source)
    });
    let n = page.features.len() as i64;
    let next = match at {
        Some(i) => (i as i64 + delta).rem_euclid(n) as usize,
        None if delta >= 0 => 0,
        None => (n - 1) as usize,
    };
    let f = &page.features[next];
    m.select(Some(FeatureRef {
        source: source.clone(),
        id: f.id.clone(),
    }));
    if let (Shape::Point(lon, lat), Some(v)) = (&f.shape, m.view) {
        let (x, y) = map::to_screen(v, *lon, *lat, m.size);
        let inside = x > 20.0 && y > 20.0 && x < m.size.0 - 20.0 && y < m.size.1 - 20.0;
        if !inside {
            m.view = Some(map::centre_on(v, *lon, *lat));
            m.moved = true;
        }
    }
}

/// Begin a rectangle at a screen point.
pub fn start_rectangle(mut map_ui: Signal<MapUi>, px: f64, py: f64) {
    let mut m = map_ui.write();
    let Some(v) = m.view else { return };
    let at = map::from_screen(v, px, py, m.size);
    m.draft = Some(Draft::Rectangle {
        from: at,
        to: at,
        done: false,
    });
}

/// Stretch the rectangle to a screen point.
pub fn drag_rectangle(mut map_ui: Signal<MapUi>, px: f64, py: f64) {
    let mut m = map_ui.write();
    let (view, size) = (m.view, m.size);
    if let (Some(v), Some(Draft::Rectangle { to, .. })) = (view, &mut m.draft) {
        *to = map::from_screen(v, px, py, size);
    }
}

/// End the rectangle. One with no area is dropped.
pub fn finish_rectangle(mut map_ui: Signal<MapUi>) {
    let mut m = map_ui.write();
    if let Some(Draft::Rectangle { done, .. }) = &mut m.draft {
        *done = true;
    }
    if m.draft.as_ref().and_then(Draft::ring).is_none() {
        m.draft = None;
    }
}

/// Add a polygon vertex at a screen point. A finished polygon starts over.
pub fn add_vertex(mut map_ui: Signal<MapUi>, px: f64, py: f64) {
    let mut m = map_ui.write();
    let Some(v) = m.view else { return };
    let p = map::from_screen(v, px, py, m.size);
    match &mut m.draft {
        Some(Draft::Polygon {
            points,
            done: false,
        }) => points.push(p),
        _ => {
            m.draft = Some(Draft::Polygon {
                points: vec![p],
                done: false,
            });
        }
    }
}

/// Close the polygon once it has three vertices.
pub fn finish_polygon(mut map_ui: Signal<MapUi>) {
    let mut m = map_ui.write();
    if let Some(Draft::Polygon { points, done }) = &mut m.draft
        && points.len() >= 3
    {
        *done = true;
    }
}

/// The two clicks of a double click each added a vertex at one spot. Keep one.
pub fn drop_double_click_vertices(mut map_ui: Signal<MapUi>) {
    let mut m = map_ui.write();
    if let Some(Draft::Polygon { points, .. }) = &mut m.draft {
        while points.len() >= 2 && points[points.len() - 1] == points[points.len() - 2] {
            points.pop();
        }
    }
}

/// Add a measurement point. A third point starts a new measurement.
pub fn add_measure_point(mut map_ui: Signal<MapUi>, px: f64, py: f64) {
    let mut m = map_ui.write();
    let Some(v) = m.view else { return };
    let at = map::from_screen(v, px, py, m.size);
    if m.measure.len() >= 2 {
        m.measure.clear();
    }
    m.measure.push(at);
}

/// Escape: drop the draft and the measurement.
pub fn cancel_drawing(mut map_ui: Signal<MapUi>) {
    let mut m = map_ui.write();
    m.draft = None;
    m.measure.clear();
}
