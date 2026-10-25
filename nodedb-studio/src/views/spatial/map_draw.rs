//! Map drawing math: where features, clusters, the graticule, and shapes sit
//! on the screen. Plain functions, so the canvas and its picking share one
//! layout.

use std::collections::BTreeMap;

use crate::models::geo_ops::{Ring, Shape};
use crate::models::geo_proj::{PlaneView, degree_label, map, ticks};
use crate::models::spatial::{Feature, FeatureRef, LayerPage, SourceRef};

/// Markers closer than this, in pixels, merge into one cluster.
pub const CLUSTER_PX: f64 = 16.0;
/// The distance within which a pointer picks a marker, in pixels.
pub const PICK_PX: f64 = 12.0;
/// Below this ground scale a cluster is split by selection, not zoom.
pub const SPLIT_RES: f64 = 0.5;

/// One feature drawn as a point.
#[derive(Debug, Clone, PartialEq)]
pub struct Marker {
    pub feature: FeatureRef,
    pub title: String,
    pub x: f64,
    pub y: f64,
}

/// What the map draws for point features.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Point(Marker),
    /// Features too close to tell apart at this scale.
    Cluster {
        x: f64,
        y: f64,
        members: Vec<Marker>,
    },
}

impl Item {
    pub fn at(&self) -> (f64, f64) {
        match self {
            Item::Point(m) => (m.x, m.y),
            Item::Cluster { x, y, .. } => (*x, *y),
        }
    }
}

fn marker(source: &SourceRef, f: &Feature, x: f64, y: f64) -> Marker {
    Marker {
        feature: FeatureRef {
            source: source.clone(),
            id: f.id.clone(),
        },
        title: f.title.clone(),
        x,
        y,
    }
}

/// The point features of the visible pages as markers and clusters. A
/// feature off screen is dropped. The selected one is never merged, so it
/// can always be seen.
pub fn layout(
    view: PlaneView,
    size: (f64, f64),
    pages: &[&LayerPage],
    selected: Option<&FeatureRef>,
) -> Vec<Item> {
    let margin = 24.0;
    let mut cells: BTreeMap<(i64, i64), Vec<Marker>> = BTreeMap::new();
    let mut items = Vec::new();
    for page in pages {
        for f in &page.features {
            let Shape::Point(lon, lat) = f.shape else {
                continue;
            };
            let (x, y) = map::to_screen(view, lon, lat, size);
            if x < -margin || y < -margin || x > size.0 + margin || y > size.1 + margin {
                continue;
            }
            let m = marker(&page.source, f, x, y);
            if selected == Some(&m.feature) {
                items.push(Item::Point(m));
                continue;
            }
            let cell = (
                (x / CLUSTER_PX).floor() as i64,
                (y / CLUSTER_PX).floor() as i64,
            );
            cells.entry(cell).or_default().push(m);
        }
    }
    for (_, mut members) in cells {
        if members.len() == 1 {
            items.extend(members.pop().map(Item::Point));
        } else {
            let n = members.len() as f64;
            let x = members.iter().map(|m| m.x).sum::<f64>() / n;
            let y = members.iter().map(|m| m.y).sum::<f64>() / n;
            items.push(Item::Cluster { x, y, members });
        }
    }
    items
}

/// The item nearest `(px, py)` within the pick distance.
pub fn pick(items: &[Item], px: f64, py: f64) -> Option<&Item> {
    items
        .iter()
        .map(|i| {
            let (x, y) = i.at();
            ((x - px).hypot(y - py), i)
        })
        .filter(|(d, _)| *d <= PICK_PX)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, i)| i)
}

/// An SVG path for one ring.
pub fn ring_path(view: PlaneView, size: (f64, f64), ring: &Ring) -> String {
    let mut d = String::new();
    for (i, (lon, lat)) in ring.iter().enumerate() {
        let (x, y) = map::to_screen(view, *lon, *lat, size);
        d.push_str(&format!("{}{x:.1} {y:.1}", if i == 0 { "M" } else { "L" }));
    }
    d.push('Z');
    d
}

/// An SVG path for a polygon. Fill it with the even-odd rule, so a hole
/// stays open.
pub fn polygon_path(view: PlaneView, size: (f64, f64), rings: &[Ring]) -> String {
    rings
        .iter()
        .map(|r| ring_path(view, size, r))
        .collect::<Vec<_>>()
        .join(" ")
}

/// A graticule line and its label.
#[derive(Debug, Clone, PartialEq)]
pub struct Tick {
    /// The line's screen position: `x` for a meridian, `y` for a parallel.
    pub at: f64,
    pub label: String,
}

/// The meridians and parallels inside the view, on round degree values. The
/// labels are computed from the same projection the features use.
pub fn graticule(view: PlaneView, size: (f64, f64)) -> (Vec<Tick>, Vec<Tick>) {
    let b = map::bounds(view, size);
    let columns = ((size.0 / 150.0).floor() as usize).clamp(2, 8);
    let rows = ((size.1 / 110.0).floor() as usize).clamp(2, 6);
    let (lons, lon_step) = ticks(b.west, b.east, columns);
    let (lats, lat_step) = ticks(b.south, b.north, rows);
    let mid_lat = (b.south + b.north) / 2.0;
    let mid_lon = (b.west + b.east) / 2.0;
    let meridians = lons
        .into_iter()
        .map(|lon| Tick {
            at: map::to_screen(view, lon, mid_lat, size).0,
            label: degree_label(lon, lon_step, true),
        })
        .collect();
    let parallels = lats
        .into_iter()
        .map(|lat| Tick {
            at: map::to_screen(view, mid_lon, lat, size).1,
            label: degree_label(lat, lat_step, false),
        })
        .collect();
    (meridians, parallels)
}

/// The width in pixels of `text` at the map's label size, roughly.
pub fn text_width(text: &str) -> f64 {
    text.chars().count() as f64 * 7.4 + 18.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::geo_ops::Bbox;

    fn page(points: &[(&str, f64, f64)]) -> LayerPage {
        LayerPage {
            source: SourceRef::new("places", "geometry"),
            features: points
                .iter()
                .map(|(id, x, y)| Feature {
                    id: (*id).to_string(),
                    title: (*id).to_string(),
                    props: Vec::new(),
                    shape: Shape::Point(*x, *y),
                })
                .collect(),
            matching: points.len() as u64,
        }
    }

    fn view() -> (PlaneView, (f64, f64)) {
        let size = (900.0, 520.0);
        (
            map::fit(Bbox::new(-74.02, 40.69, -73.9, 40.8), size, 24.0),
            size,
        )
    }

    #[test]
    fn far_apart_points_stay_points_and_near_ones_cluster() {
        let (v, s) = view();
        let p = page(&[
            ("a", -74.01, 40.70),
            ("b", -73.91, 40.79),
            ("c", -73.9600, 40.7500),
            ("d", -73.96001, 40.75001),
        ]);
        let items = layout(v, s, &[&p], None);
        assert_eq!(items.len(), 3);
        let clusters: Vec<usize> = items
            .iter()
            .filter_map(|i| match i {
                Item::Cluster { members, .. } => Some(members.len()),
                Item::Point(_) => None,
            })
            .collect();
        assert_eq!(clusters, [2]);
    }

    #[test]
    fn the_selected_feature_is_never_merged() {
        let (v, s) = view();
        let p = page(&[("c", -73.96, 40.75), ("d", -73.96001, 40.75001)]);
        let sel = FeatureRef {
            source: SourceRef::new("places", "geometry"),
            id: "c".into(),
        };
        let items = layout(v, s, &[&p], Some(&sel));
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|i| matches!(i, Item::Point(_))));
    }

    #[test]
    fn a_point_off_screen_is_not_drawn() {
        let (v, s) = view();
        let p = page(&[("far", 10.0, 10.0), ("near", -73.96, 40.75)]);
        assert_eq!(layout(v, s, &[&p], None).len(), 1);
    }

    #[test]
    fn picking_takes_the_nearest_item_inside_the_radius() {
        let (v, s) = view();
        let p = page(&[("a", -74.01, 40.70), ("b", -73.91, 40.79)]);
        let items = layout(v, s, &[&p], None);
        let (ax, ay) = map::to_screen(v, -74.01, 40.70, s);
        let hit = pick(&items, ax + 5.0, ay - 4.0).expect("hit");
        assert!(matches!(hit, Item::Point(m) if m.feature.id == "a"));
        assert!(pick(&items, ax + 60.0, ay).is_none());
    }

    #[test]
    fn a_ring_path_closes_and_a_hole_adds_a_subpath() {
        let (v, s) = view();
        let outer = Bbox::new(-74.0, 40.7, -73.92, 40.78).ring();
        let hole = Bbox::new(-73.98, 40.72, -73.95, 40.75).ring();
        let d = polygon_path(v, s, &[outer, hole]);
        assert_eq!(d.matches('M').count(), 2);
        assert_eq!(d.matches('Z').count(), 2);
    }

    #[test]
    fn graticule_lines_sit_on_their_degree_values() {
        let (v, s) = view();
        let (meridians, parallels) = graticule(v, s);
        assert!(!meridians.is_empty() && !parallels.is_empty());
        for t in &meridians {
            assert!(t.at >= 0.0 && t.at <= s.0, "{t:?}");
            assert!(t.label.ends_with('W'), "{}", t.label);
            // The label's value maps back to the line's screen position.
            let lon: f64 = t.label.trim_end_matches("°W").parse().expect("number");
            let (x, _) = map::to_screen(v, -lon, 40.745, s);
            assert!(
                (x - t.at).abs() < 0.5,
                "{} at {}, expected {x}",
                t.label,
                t.at
            );
        }
        for t in &parallels {
            assert!(t.label.ends_with('N'));
            let lat: f64 = t.label.trim_end_matches("°N").parse().expect("number");
            let (_, y) = map::to_screen(v, -73.96, lat, s);
            assert!(
                (y - t.at).abs() < 0.5,
                "{} at {}, expected {y}",
                t.label,
                t.at
            );
        }
    }
}
