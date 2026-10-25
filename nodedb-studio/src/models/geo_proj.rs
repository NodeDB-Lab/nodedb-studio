//! Web Mercator (EPSG:3857) and the map window built on it, in plain Rust.
//!
//! A `PlaneView` is a window onto any plane: a centre and a scale in plane
//! units per screen pixel. The map uses it on the Mercator plane, and the
//! raster canvas uses it on the raster's own plane. Screen `y` grows down and
//! plane `y` grows up.

use std::f64::consts::PI;

use crate::models::geo_ops::Bbox;

/// WGS 84 semi-major axis in metres. Web Mercator treats it as a sphere.
pub const RADIUS: f64 = 6_378_137.0;
/// Web Mercator ends here. Latitudes beyond it have no position.
pub const MAX_LAT: f64 = 85.051_128_779_806_59;

/// `(lon, lat)` in degrees to Web Mercator metres. Latitude is clamped to
/// the projection's domain.
pub fn mercator(lon: f64, lat: f64) -> (f64, f64) {
    let lat = lat.clamp(-MAX_LAT, MAX_LAT);
    (
        RADIUS * lon.to_radians(),
        RADIUS * (PI / 4.0 + lat.to_radians() / 2.0).tan().ln(),
    )
}

/// Web Mercator metres back to `(lon, lat)` in degrees.
pub fn unproject(x: f64, y: f64) -> (f64, f64) {
    (
        (x / RADIUS).to_degrees(),
        (2.0 * (y / RADIUS).exp().atan() - PI / 2.0).to_degrees(),
    )
}

/// The finest scale a window takes, plane units per pixel.
const RES_MIN: f64 = 1e-6;

/// A window onto a plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaneView {
    /// The plane point at the middle of the window.
    pub cx: f64,
    pub cy: f64,
    /// Plane units per screen pixel.
    pub res: f64,
}

impl PlaneView {
    /// The window that frames `(min_x, min_y, max_x, max_y)` in a `w` by `h`
    /// pixel area, with `pad` pixels free on every side. A box with no size
    /// frames at `fallback_res`.
    pub fn fit(
        (min_x, min_y, max_x, max_y): (f64, f64, f64, f64),
        (w, h): (f64, f64),
        pad: f64,
        fallback_res: f64,
    ) -> Self {
        let (aw, ah) = ((w - 2.0 * pad).max(1.0), (h - 2.0 * pad).max(1.0));
        let need = ((max_x - min_x) / aw).max((max_y - min_y) / ah);
        Self {
            cx: (min_x + max_x) / 2.0,
            cy: (min_y + max_y) / 2.0,
            res: if need > RES_MIN { need } else { fallback_res },
        }
    }

    /// Screen pixel of plane point `(x, y)`.
    pub fn to_screen(self, x: f64, y: f64, (w, h): (f64, f64)) -> (f64, f64) {
        (
            (x - self.cx) / self.res + w / 2.0,
            (self.cy - y) / self.res + h / 2.0,
        )
    }

    /// Plane point under screen pixel `(px, py)`.
    #[allow(clippy::wrong_self_convention)] // the pair of `to_screen`
    pub fn from_screen(self, px: f64, py: f64, (w, h): (f64, f64)) -> (f64, f64) {
        (
            self.cx + (px - w / 2.0) * self.res,
            self.cy - (py - h / 2.0) * self.res,
        )
    }

    /// The window's plane extent as `(min_x, min_y, max_x, max_y)`.
    pub fn extent(self, size: (f64, f64)) -> (f64, f64, f64, f64) {
        let (x0, y1) = self.from_screen(0.0, 0.0, size);
        let (x1, y0) = self.from_screen(size.0, size.1, size);
        (x0, y0, x1, y1)
    }

    /// Zoom in by `factor` (above 1) or out (below 1), keeping the plane
    /// point under `(px, py)` where it is. `res` stays within `[min, max]`.
    pub fn zoomed_about(
        self,
        factor: f64,
        (px, py): (f64, f64),
        size: (f64, f64),
        (min, max): (f64, f64),
    ) -> Self {
        let res = (self.res / factor).clamp(min.max(RES_MIN), max);
        let (x, y) = self.from_screen(px, py, size);
        let moved = Self { res, ..self };
        let (nx, ny) = moved.from_screen(px, py, size);
        Self {
            cx: self.cx + (x - nx),
            cy: self.cy + (y - ny),
            res,
        }
    }

    /// Move the window so the content follows a drag of `(dx, dy)` pixels.
    pub fn panned_px(self, dx: f64, dy: f64) -> Self {
        Self {
            cx: self.cx - dx * self.res,
            cy: self.cy + dy * self.res,
            ..self
        }
    }

    pub fn centered(self, x: f64, y: f64) -> Self {
        Self {
            cx: x,
            cy: y,
            ..self
        }
    }
}

/// The map window on the Mercator plane.
pub mod map {
    use super::*;

    /// The coarsest scale: the whole world across 256 pixels.
    pub const RES_MAX: f64 = 2.0 * PI * RADIUS / 256.0;
    /// The finest scale: a pixel is a decimetre.
    pub const RES_MIN_MAP: f64 = 0.1;

    pub fn limits() -> (f64, f64) {
        (RES_MIN_MAP, RES_MAX)
    }

    /// The window that frames a box of degrees.
    pub fn fit(bbox: Bbox, size: (f64, f64), pad: f64) -> PlaneView {
        let (x0, y0) = mercator(bbox.west, bbox.south);
        let (x1, y1) = mercator(bbox.east, bbox.north);
        PlaneView::fit((x0, y0, x1, y1), size, pad, 2.0)
    }

    pub fn centre_on(view: PlaneView, lon: f64, lat: f64) -> PlaneView {
        let (x, y) = mercator(lon, lat);
        view.centered(x, y)
    }

    /// Screen pixel of a `(lon, lat)`.
    pub fn to_screen(view: PlaneView, lon: f64, lat: f64, size: (f64, f64)) -> (f64, f64) {
        let (x, y) = mercator(lon, lat);
        view.to_screen(x, y, size)
    }

    /// `(lon, lat)` under a screen pixel.
    pub fn from_screen(view: PlaneView, px: f64, py: f64, size: (f64, f64)) -> (f64, f64) {
        let (x, y) = view.from_screen(px, py, size);
        let (lon, lat) = unproject(x, y);
        (wrap_lon(lon), lat)
    }

    /// What the window shows, in degrees. Longitude past ±180 clamps: a
    /// window wider than the world asks for the whole world.
    pub fn bounds(view: PlaneView, size: (f64, f64)) -> Bbox {
        let (x0, y0, x1, y1) = view.extent(size);
        let (west, south) = unproject(x0, y0);
        let (east, north) = unproject(x1, y1);
        Bbox::new(west.max(-180.0), south, east.min(180.0), north)
    }

    /// Metres on the ground per screen pixel at `lat`.
    pub fn metres_per_px(view: PlaneView, lat: f64) -> f64 {
        view.res * lat.to_radians().cos()
    }

    fn wrap_lon(lon: f64) -> f64 {
        (lon + 540.0).rem_euclid(360.0) - 180.0
    }
}

/// A scale bar: the longest of 1, 2, or 5 times a power of ten metres that
/// fits `max_px`, and its length in pixels.
pub fn scale_bar(metres_per_px: f64, max_px: f64) -> Option<(f64, f64)> {
    if !(metres_per_px.is_finite() && metres_per_px > 0.0) || max_px <= 0.0 {
        return None;
    }
    let limit = metres_per_px * max_px;
    let mut best = None;
    for exp in -3..=8 {
        for m in [1.0, 2.0, 5.0] {
            let metres = m * 10f64.powi(exp);
            if metres <= limit {
                best = Some(metres);
            }
        }
    }
    best.map(|m| (m, m / metres_per_px))
}

/// `500 m`, `2 km`.
pub fn length_label(metres: f64) -> String {
    if metres >= 1000.0 {
        format!("{} km", trim(metres / 1000.0))
    } else if metres >= 1.0 {
        format!("{} m", trim(metres))
    } else {
        format!("{} cm", trim(metres * 100.0))
    }
}

fn trim(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Axis tick values inside `[lo, hi]`, on a step of 1, 2, or 5 times a power
/// of ten, at most `max` of them.
pub fn ticks(lo: f64, hi: f64, max: usize) -> (Vec<f64>, f64) {
    let range = hi - lo;
    if !(range.is_finite() && range > 0.0) || max == 0 {
        return (Vec::new(), 0.0);
    }
    let raw = range / max as f64;
    let exp = raw.log10().floor();
    let base = 10f64.powf(exp);
    let step = [1.0, 2.0, 5.0, 10.0]
        .iter()
        .map(|m| m * base)
        .find(|s| *s >= raw)
        .unwrap_or(10.0 * base);
    let first = (lo / step).ceil() as i64;
    let last = (hi / step).floor() as i64;
    let values = (first..=last).map(|i| i as f64 * step).collect();
    (values, step)
}

/// A degree label with its hemisphere: `73.99°W`, `40.75°N`. `step` sets the
/// decimals, so every label on an axis has the same.
pub fn degree_label(value: f64, step: f64, longitude: bool) -> String {
    let decimals = if step >= 1.0 {
        0
    } else {
        (-step.log10().floor()) as usize
    };
    let hemi = match (longitude, value < 0.0) {
        (true, true) => "W",
        (true, false) => "E",
        (false, true) => "S",
        (false, false) => "N",
    };
    let shown = if value == 0.0 { 0.0 } else { value.abs() };
    format!("{shown:.decimals$}°{hemi}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mercator_matches_published_values() {
        let (x, y) = mercator(0.0, 0.0);
        assert!(x.abs() < 1e-9 && y.abs() < 1e-9);
        // 180 degrees east is half the world's circumference.
        let (x, _) = mercator(180.0, 0.0);
        assert!((x - 20_037_508.342_789_244).abs() < 1e-6, "{x}");
        // New York, from the standard EPSG:3857 conversion.
        let (x, y) = mercator(-74.0060, 40.7128);
        assert!((x - -8_238_310.24).abs() < 1.0, "{x}");
        assert!((y - 4_970_071.58).abs() < 1.0, "{y}");
    }

    #[test]
    fn the_projection_round_trips() {
        for (lon, lat) in [
            (-73.9857, 40.7484),
            (101.713, 3.158),
            (179.9, -84.0),
            (0.0, 0.0),
        ] {
            let (x, y) = mercator(lon, lat);
            let (l, a) = unproject(x, y);
            assert!(
                (l - lon).abs() < 1e-9 && (a - lat).abs() < 1e-9,
                "{lon} {lat}"
            );
        }
    }

    #[test]
    fn latitude_past_the_domain_clamps() {
        let (_, y) = mercator(0.0, 89.9);
        let (_, edge) = mercator(0.0, MAX_LAT);
        assert_eq!(y, edge);
    }

    #[test]
    fn north_is_up_and_east_is_right_on_screen() {
        let size = (800.0, 500.0);
        let view = map::fit(Bbox::new(-74.0, 40.7, -73.9, 40.8), size, 20.0);
        let sw = map::to_screen(view, -74.0, 40.7, size);
        let ne = map::to_screen(view, -73.9, 40.8, size);
        assert!(ne.0 > sw.0, "east is right");
        assert!(ne.1 < sw.1, "north is up");
    }

    #[test]
    fn screen_and_map_round_trip() {
        let size = (900.0, 520.0);
        let view = map::fit(Bbox::new(-74.02, 40.69, -73.9, 40.8), size, 24.0);
        let (px, py) = map::to_screen(view, -73.9857, 40.7484, size);
        let (lon, lat) = map::from_screen(view, px, py, size);
        assert!((lon - -73.9857).abs() < 1e-9 && (lat - 40.7484).abs() < 1e-9);
    }

    #[test]
    fn a_fitted_box_sits_inside_the_padding() {
        let size = (900.0, 520.0);
        let b = Bbox::new(-74.02, 40.69, -73.9, 40.8);
        let view = map::fit(b, size, 24.0);
        for (lon, lat) in [(b.west, b.south), (b.east, b.north)] {
            let (x, y) = map::to_screen(view, lon, lat, size);
            assert!(
                (23.9..=876.1).contains(&x) && (23.9..=496.1).contains(&y),
                "{x} {y}"
            );
        }
    }

    #[test]
    fn zooming_about_a_pixel_keeps_the_point_under_it() {
        let size = (900.0, 520.0);
        let view = map::fit(Bbox::new(-74.02, 40.69, -73.9, 40.8), size, 24.0);
        let at = (610.0, 140.0);
        let before = map::from_screen(view, at.0, at.1, size);
        let zoomed = view.zoomed_about(2.0, at, size, map::limits());
        let after = map::from_screen(zoomed, at.0, at.1, size);
        assert!((before.0 - after.0).abs() < 1e-9 && (before.1 - after.1).abs() < 1e-9);
        assert!((zoomed.res - view.res / 2.0).abs() < 1e-9);
    }

    #[test]
    fn zoom_stops_at_its_limits() {
        let size = (900.0, 520.0);
        let mut view = map::fit(Bbox::new(-74.0, 40.7, -73.9, 40.8), size, 20.0);
        for _ in 0..60 {
            view = view.zoomed_about(2.0, (450.0, 260.0), size, map::limits());
        }
        assert_eq!(view.res, map::RES_MIN_MAP);
        for _ in 0..60 {
            view = view.zoomed_about(0.5, (450.0, 260.0), size, map::limits());
        }
        assert_eq!(view.res, map::RES_MAX);
    }

    #[test]
    fn a_drag_moves_the_content_with_the_pointer() {
        let size = (900.0, 520.0);
        let view = map::fit(Bbox::new(-74.0, 40.7, -73.9, 40.8), size, 20.0);
        let before = map::to_screen(view, -73.95, 40.75, size);
        let dragged = view.panned_px(30.0, -12.0);
        let after = map::to_screen(dragged, -73.95, 40.75, size);
        assert!((after.0 - before.0 - 30.0).abs() < 1e-6);
        assert!((after.1 - before.1 + 12.0).abs() < 1e-6);
    }

    #[test]
    fn bounds_cover_the_window_corners() {
        let size = (900.0, 520.0);
        let view = map::fit(Bbox::new(-74.0, 40.7, -73.9, 40.8), size, 20.0);
        let b = map::bounds(view, size);
        assert!(b.west < -74.0 && b.east > -73.9 && b.south < 40.7 && b.north > 40.8);
    }

    #[test]
    fn ground_scale_shrinks_with_latitude() {
        let view = PlaneView {
            cx: 0.0,
            cy: 0.0,
            res: 10.0,
        };
        assert!((map::metres_per_px(view, 0.0) - 10.0).abs() < 1e-9);
        assert!((map::metres_per_px(view, 60.0) - 5.0).abs() < 1e-9);
    }

    #[test]
    fn a_scale_bar_is_a_round_length_that_fits() {
        let (m, px) = scale_bar(2.4, 120.0).expect("bar");
        assert_eq!(m, 200.0);
        assert!((px - 200.0 / 2.4).abs() < 1e-9 && px <= 120.0);
        let (m, _) = scale_bar(0.9, 100.0).expect("bar");
        assert_eq!(m, 50.0);
        assert!(scale_bar(0.0, 100.0).is_none());
        assert_eq!(length_label(500.0), "500 m");
        assert_eq!(length_label(2000.0), "2 km");
        assert_eq!(length_label(1500.0), "1.5 km");
    }

    #[test]
    fn ticks_land_on_round_values_inside_the_range() {
        let (t, step) = ticks(-74.02, -73.9, 5);
        assert!((step - 0.05).abs() < 1e-12, "{step}");
        assert!(t.iter().all(|v| *v >= -74.02 - 1e-9 && *v <= -73.9 + 1e-9));
        assert!(t.len() >= 2 && t.len() <= 5, "{t:?}");
        let (t, step) = ticks(40.69, 40.8, 4);
        assert!(step > 0.0 && !t.is_empty());
        assert!(ticks(1.0, 1.0, 4).0.is_empty());
    }

    #[test]
    fn degree_labels_carry_the_hemisphere() {
        assert_eq!(degree_label(-73.95, 0.05, true), "73.95°W");
        assert_eq!(degree_label(40.75, 0.05, false), "40.75°N");
        assert_eq!(degree_label(101.7, 0.1, true), "101.7°E");
        assert_eq!(degree_label(-3.0, 1.0, false), "3°S");
        assert_eq!(degree_label(0.0, 1.0, true), "0°E");
    }
}
