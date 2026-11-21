//! Plain-Rust geometry for the Spatial screens: boxes, shapes, point
//! location, geodesic distance, buffers, validity, and text formats.
//!
//! Every coordinate is `(longitude, latitude)` in degrees. The order never
//! flips: a pair is x then y, as GeoJSON and `ST_Point(lon, lat)` write it.
//! Distances are metres on a sphere of the mean Earth radius. A server
//! measures on the CRS ellipsoid, so a server answer can differ by a few
//! tenths of a percent.

/// Mean Earth radius in metres.
pub const EARTH_RADIUS_M: f64 = 6_371_008.8;

/// One ring of a polygon. Closed: the last point repeats the first.
pub type Ring = Vec<(f64, f64)>;

/// A box in degrees. `west > east` means the box crosses the antimeridian.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bbox {
    pub west: f64,
    pub south: f64,
    pub east: f64,
    pub north: f64,
}

impl Bbox {
    pub const fn new(west: f64, south: f64, east: f64, north: f64) -> Self {
        Self {
            west,
            south,
            east,
            north,
        }
    }

    /// The smallest box that holds every point. `None` for no points.
    pub fn from_points(points: impl IntoIterator<Item = (f64, f64)>) -> Option<Self> {
        let mut it = points.into_iter();
        let (x, y) = it.next()?;
        let mut b = Self::new(x, y, x, y);
        for (x, y) in it {
            b.west = b.west.min(x);
            b.east = b.east.max(x);
            b.south = b.south.min(y);
            b.north = b.north.max(y);
        }
        Some(b)
    }

    pub fn crosses_antimeridian(self) -> bool {
        self.west > self.east
    }

    /// The boxes a query runs, one per side of the antimeridian when the box
    /// crosses it. A box that does not cross is its own part.
    pub fn parts(self) -> Vec<Bbox> {
        if self.crosses_antimeridian() {
            vec![
                Bbox::new(self.west, self.south, 180.0, self.north),
                Bbox::new(-180.0, self.south, self.east, self.north),
            ]
        } else {
            vec![self]
        }
    }

    /// Whether the point lies in the box. The edge counts.
    pub fn contains(self, lon: f64, lat: f64) -> bool {
        self.parts()
            .iter()
            .any(|b| lon >= b.west && lon <= b.east && lat >= b.south && lat <= b.north)
    }

    /// Whether the two boxes share any point. The edge counts.
    pub fn intersects(self, other: Bbox) -> bool {
        self.parts().iter().any(|a| {
            other.parts().iter().any(|b| {
                a.west <= b.east && b.west <= a.east && a.south <= b.north && b.south <= a.north
            })
        })
    }

    /// The box grown by `fraction` of its size on every side.
    pub fn grown(self, fraction: f64) -> Bbox {
        let (dx, dy) = (
            (self.east - self.west) * fraction,
            (self.north - self.south) * fraction,
        );
        Bbox::new(
            self.west - dx,
            self.south - dy,
            self.east + dx,
            self.north + dy,
        )
    }

    /// The smallest box that holds both. Neither may cross the antimeridian.
    pub fn union(self, other: Bbox) -> Bbox {
        Bbox::new(
            self.west.min(other.west),
            self.south.min(other.south),
            self.east.max(other.east),
            self.north.max(other.north),
        )
    }

    /// The box that holds every point within `metres` of `(lon, lat)`.
    pub fn around(lon: f64, lat: f64, metres: f64) -> Bbox {
        let north = destination((lon, lat), 0.0, metres).1;
        let south = destination((lon, lat), 180.0, metres).1;
        let east = destination((lon, lat), 90.0, metres).0;
        let west = destination((lon, lat), 270.0, metres).0;
        Bbox::new(west, south, east, north)
    }

    /// The box as a closed counter-clockwise ring.
    pub fn ring(self) -> Ring {
        vec![
            (self.west, self.south),
            (self.east, self.south),
            (self.east, self.north),
            (self.west, self.north),
            (self.west, self.south),
        ]
    }
}

/// A geometry Studio draws and inspects. A polygon's first ring is its shell
/// and every later ring is a hole.
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    Point(f64, f64),
    Polygon(Vec<Ring>),
}

impl Shape {
    pub fn kind(&self) -> &'static str {
        match self {
            Shape::Point(..) => "Point",
            Shape::Polygon(_) => "Polygon",
        }
    }

    pub fn bbox(&self) -> Option<Bbox> {
        match self {
            Shape::Point(x, y) => Some(Bbox::new(*x, *y, *x, *y)),
            Shape::Polygon(rings) => Bbox::from_points(rings.iter().flatten().copied()),
        }
    }

    /// Whether the shape touches a box. A point must lie in it.
    pub fn touches(&self, b: Bbox) -> bool {
        match self {
            Shape::Point(x, y) => b.contains(*x, *y),
            Shape::Polygon(_) => self.bbox().is_some_and(|sb| b.intersects(sb)),
        }
    }

    /// The count of vertices, closing points included.
    pub fn vertices(&self) -> usize {
        match self {
            Shape::Point(..) => 1,
            Shape::Polygon(rings) => rings.iter().map(Vec::len).sum(),
        }
    }

    /// `POINT(-73.9857 40.7484)`. Longitude first.
    pub fn wkt(&self) -> String {
        match self {
            Shape::Point(x, y) => format!("POINT({x} {y})"),
            Shape::Polygon(rings) => format!("POLYGON({})", ring_list(rings, " ", ", ")),
        }
    }

    /// `SRID=4326;POINT(-73.9857 40.7484)`.
    pub fn ewkt(&self, srid: u32) -> String {
        format!("SRID={srid};{}", self.wkt())
    }

    /// GeoJSON geometry text, longitude first.
    pub fn geojson(&self) -> String {
        match self {
            Shape::Point(x, y) => format!("{{\"type\":\"Point\",\"coordinates\":[{x},{y}]}}"),
            Shape::Polygon(rings) => {
                let rings: Vec<String> = rings
                    .iter()
                    .map(|r| {
                        let pts: Vec<String> =
                            r.iter().map(|(x, y)| format!("[{x},{y}]")).collect();
                        format!("[{}]", pts.join(","))
                    })
                    .collect();
                format!(
                    "{{\"type\":\"Polygon\",\"coordinates\":[{}]}}",
                    rings.join(",")
                )
            }
        }
    }
}

fn ring_list(rings: &[Ring], pair: &str, sep: &str) -> String {
    let rings: Vec<String> = rings
        .iter()
        .map(|r| {
            let pts: Vec<String> = r.iter().map(|(x, y)| format!("{x}{pair}{y}")).collect();
            format!("({})", pts.join(sep))
        })
        .collect();
    rings.join(", ")
}

/// Where a point lies against a ring or polygon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Location {
    Inside,
    Boundary,
    Outside,
}

fn cross(a: (f64, f64), b: (f64, f64), p: (f64, f64)) -> f64 {
    (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0)
}

fn on_segment(a: (f64, f64), b: (f64, f64), p: (f64, f64)) -> bool {
    cross(a, b, p) == 0.0
        && p.0 >= a.0.min(b.0)
        && p.0 <= a.0.max(b.0)
        && p.1 >= a.1.min(b.1)
        && p.1 <= a.1.max(b.1)
}

/// Where `p` lies against one ring. The ring may omit its closing point.
pub fn locate_in_ring(p: (f64, f64), ring: &[(f64, f64)]) -> Location {
    let n = ring.len();
    if n < 3 {
        return Location::Outside;
    }
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        if a == b {
            continue;
        }
        if on_segment(a, b, p) {
            return Location::Boundary;
        }
        if (a.1 > p.1) != (b.1 > p.1) {
            let x = a.0 + (p.1 - a.1) / (b.1 - a.1) * (b.0 - a.0);
            if p.0 < x {
                inside = !inside;
            }
        }
    }
    if inside {
        Location::Inside
    } else {
        Location::Outside
    }
}

/// Where `p` lies against a polygon. A point in a hole is outside, and a
/// point on a hole's edge is on the boundary.
pub fn locate_in_polygon(p: (f64, f64), rings: &[Ring]) -> Location {
    let Some((shell, holes)) = rings.split_first() else {
        return Location::Outside;
    };
    match locate_in_ring(p, shell) {
        Location::Outside => Location::Outside,
        Location::Boundary => Location::Boundary,
        Location::Inside => {
            for hole in holes {
                match locate_in_ring(p, hole) {
                    Location::Inside => return Location::Outside,
                    Location::Boundary => return Location::Boundary,
                    Location::Outside => {}
                }
            }
            Location::Inside
        }
    }
}

/// `ST_Within(point, polygon)`: strictly inside. The edge is not within.
pub fn point_within(p: (f64, f64), rings: &[Ring]) -> bool {
    locate_in_polygon(p, rings) == Location::Inside
}

/// `ST_Intersects(point, polygon)`: inside or on the edge.
pub fn point_intersects(p: (f64, f64), rings: &[Ring]) -> bool {
    locate_in_polygon(p, rings) != Location::Outside
}

/// Great-circle distance in metres between two `(lon, lat)` points.
pub fn haversine_m(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (lat1, lat2) = (a.1.to_radians(), b.1.to_radians());
    let dlat = lat2 - lat1;
    let dlon = (b.0 - a.0).to_radians();
    let h = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_M * h.sqrt().min(1.0).asin()
}

/// The point `distance_m` from `from` along `bearing_deg` (0 is north).
pub fn destination(from: (f64, f64), bearing_deg: f64, distance_m: f64) -> (f64, f64) {
    let (lat1, lon1) = (from.1.to_radians(), from.0.to_radians());
    let (theta, delta) = (bearing_deg.to_radians(), distance_m / EARTH_RADIUS_M);
    let lat2 = (lat1.sin() * delta.cos() + lat1.cos() * delta.sin() * theta.cos()).asin();
    let lon2 = lon1
        + (theta.sin() * delta.sin() * lat1.cos()).atan2(delta.cos() - lat1.sin() * lat2.sin());
    let lon = (lon2.to_degrees() + 540.0).rem_euclid(360.0) - 180.0;
    (lon, lat2.to_degrees())
}

/// `ST_Buffer(point, radius_m)`: a ring of `segments` edges, closed, every
/// vertex `radius_m` from the centre.
pub fn buffer_ring(centre: (f64, f64), radius_m: f64, segments: usize) -> Ring {
    let n = segments.max(8);
    let mut ring: Ring = (0..n)
        .map(|i| destination(centre, 360.0 * i as f64 / n as f64, radius_m))
        .collect();
    if let Some(first) = ring.first().copied() {
        ring.push(first);
    }
    ring
}

/// The result of a validity check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Validity {
    Valid,
    Invalid(String),
}

impl Validity {
    /// The `ST_IsValidReason` text.
    pub fn reason(&self) -> String {
        match self {
            Validity::Valid => "Valid Geometry".to_string(),
            Validity::Invalid(why) => why.clone(),
        }
    }
}

fn properly_cross(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
    let (d1, d2) = (cross(a, b, c), cross(a, b, d));
    let (d3, d4) = (cross(c, d, a), cross(c, d, b));
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

fn ring_problem(ring: &[(f64, f64)], what: &str) -> Option<String> {
    if ring.len() < 4 {
        return Some(format!("{what} has too few points"));
    }
    if ring.first() != ring.last() {
        return Some(format!("{what} is not closed"));
    }
    if ring.iter().any(|(x, y)| !x.is_finite() || !y.is_finite()) {
        return Some(format!("{what} has a non-finite coordinate"));
    }
    let edges = ring.len() - 1;
    for i in 0..edges {
        for j in (i + 1)..edges {
            // Neighbouring edges share a vertex and are not a crossing.
            if j == i + 1 || (i == 0 && j == edges - 1) {
                continue;
            }
            if properly_cross(ring[i], ring[i + 1], ring[j], ring[j + 1]) {
                return Some(format!("{what} self-intersects"));
            }
        }
    }
    None
}

/// The OGC simple-feature checks Studio can make on its own: finite
/// coordinates inside the CRS domain, closed rings of four or more points,
/// no ring that crosses itself, and holes inside the shell.
pub fn validity(shape: &Shape) -> Validity {
    match shape {
        Shape::Point(x, y) => {
            if !x.is_finite() || !y.is_finite() {
                Validity::Invalid("Point has a non-finite coordinate".to_string())
            } else if !(-180.0..=180.0).contains(x) || !(-90.0..=90.0).contains(y) {
                Validity::Invalid("Point lies outside the longitude and latitude range".to_string())
            } else {
                Validity::Valid
            }
        }
        Shape::Polygon(rings) => {
            let Some((shell, holes)) = rings.split_first() else {
                return Validity::Invalid("Polygon has no shell".to_string());
            };
            if let Some(why) = ring_problem(shell, "Shell") {
                return Validity::Invalid(why);
            }
            for (i, hole) in holes.iter().enumerate() {
                if let Some(why) = ring_problem(hole, &format!("Hole {}", i + 1)) {
                    return Validity::Invalid(why);
                }
                let outside = hole
                    .iter()
                    .any(|p| locate_in_ring(*p, shell) == Location::Outside);
                if outside {
                    return Validity::Invalid(format!("Hole {} lies outside the shell", i + 1));
                }
            }
            Validity::Valid
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Ring {
        Bbox::new(0.0, 0.0, 10.0, 10.0).ring()
    }

    #[test]
    fn a_box_contains_its_edge_and_not_beyond() {
        let b = Bbox::new(-74.0, 40.7, -73.9, 40.8);
        assert!(b.contains(-73.9857, 40.7484));
        assert!(b.contains(-74.0, 40.7));
        assert!(!b.contains(-73.89, 40.75));
        assert!(!b.contains(-73.95, 40.81));
    }

    #[test]
    fn boxes_intersect_when_they_share_an_edge() {
        let a = Bbox::new(0.0, 0.0, 1.0, 1.0);
        assert!(a.intersects(Bbox::new(1.0, 1.0, 2.0, 2.0)));
        assert!(a.intersects(Bbox::new(0.5, 0.5, 0.6, 0.6)));
        assert!(!a.intersects(Bbox::new(1.1, 0.0, 2.0, 1.0)));
    }

    #[test]
    fn a_box_across_the_antimeridian_splits_in_two() {
        let b = Bbox::new(170.0, -10.0, -170.0, 10.0);
        assert!(b.crosses_antimeridian());
        assert_eq!(b.parts().len(), 2);
        assert!(b.contains(175.0, 0.0));
        assert!(b.contains(-175.0, 0.0));
        assert!(!b.contains(0.0, 0.0));
        assert!(b.intersects(Bbox::new(179.0, 0.0, 179.5, 1.0)));
        assert!(!b.intersects(Bbox::new(-10.0, 0.0, 10.0, 1.0)));
    }

    #[test]
    fn a_box_around_a_point_reaches_the_distance_on_every_side() {
        let (lon, lat) = (-73.9857, 40.7484);
        let b = Bbox::around(lon, lat, 500.0);
        assert!(b.contains(lon, lat));
        assert!((haversine_m((lon, lat), (lon, b.north)) - 500.0).abs() < 0.01);
        assert!((haversine_m((lon, lat), (b.east, lat)) - 500.0).abs() < 0.5);
        assert!(b.west < lon && b.east > lon);
    }

    #[test]
    fn bbox_from_points_spans_all_of_them() {
        let b = Bbox::from_points([(1.0, 5.0), (-2.0, 3.0), (4.0, -1.0)]).expect("box");
        assert_eq!(b, Bbox::new(-2.0, -1.0, 4.0, 5.0));
        assert!(Bbox::from_points(std::iter::empty()).is_none());
    }

    #[test]
    fn location_tells_inside_edge_and_outside_apart() {
        let rings = vec![square()];
        assert_eq!(locate_in_polygon((5.0, 5.0), &rings), Location::Inside);
        assert_eq!(locate_in_polygon((0.0, 5.0), &rings), Location::Boundary);
        assert_eq!(locate_in_polygon((10.0, 10.0), &rings), Location::Boundary);
        assert_eq!(locate_in_polygon((11.0, 5.0), &rings), Location::Outside);
        assert!(point_within((5.0, 5.0), &rings));
        assert!(!point_within((0.0, 5.0), &rings), "the edge is not within");
        assert!(point_intersects((0.0, 5.0), &rings), "the edge intersects");
    }

    #[test]
    fn a_hole_removes_its_interior_from_the_polygon() {
        let hole: Ring = Bbox::new(4.0, 4.0, 6.0, 6.0).ring();
        let rings = vec![square(), hole];
        assert_eq!(locate_in_polygon((5.0, 5.0), &rings), Location::Outside);
        assert_eq!(locate_in_polygon((4.0, 5.0), &rings), Location::Boundary);
        assert_eq!(locate_in_polygon((2.0, 2.0), &rings), Location::Inside);
    }

    #[test]
    fn a_concave_polygon_is_located_correctly() {
        // An L shape: the notch at the top right is outside.
        let l: Ring = vec![
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 4.0),
            (4.0, 4.0),
            (4.0, 10.0),
            (0.0, 10.0),
            (0.0, 0.0),
        ];
        assert!(point_within((2.0, 8.0), std::slice::from_ref(&l)));
        assert!(point_within((8.0, 2.0), std::slice::from_ref(&l)));
        assert!(!point_intersects((8.0, 8.0), &[l]));
    }

    #[test]
    fn haversine_matches_known_distances() {
        // One degree of latitude is about 111.2 km.
        let d = haversine_m((0.0, 0.0), (0.0, 1.0));
        assert!((d - 111_195.0).abs() < 50.0, "{d}");
        // New York to London, about 5,570 km.
        let ny = (-74.006, 40.7128);
        let london = (-0.1278, 51.5074);
        let d = haversine_m(ny, london);
        assert!((d - 5_570_000.0).abs() < 20_000.0, "{d}");
        assert_eq!(haversine_m(ny, ny), 0.0);
    }

    #[test]
    fn a_degree_of_longitude_shrinks_with_latitude() {
        let at_equator = haversine_m((0.0, 0.0), (1.0, 0.0));
        let at_60 = haversine_m((0.0, 60.0), (1.0, 60.0));
        assert!((at_60 / at_equator - 0.5).abs() < 0.01);
    }

    #[test]
    fn longitude_comes_before_latitude_in_every_text_format() {
        let p = Shape::Point(-73.9857, 40.7484);
        assert_eq!(p.wkt(), "POINT(-73.9857 40.7484)");
        assert_eq!(p.ewkt(4326), "SRID=4326;POINT(-73.9857 40.7484)");
        assert_eq!(
            p.geojson(),
            "{\"type\":\"Point\",\"coordinates\":[-73.9857,40.7484]}"
        );
    }

    #[test]
    fn a_buffer_ring_is_closed_and_every_vertex_is_the_radius_away() {
        let c = (-73.9857, 40.7484);
        let ring = buffer_ring(c, 1000.0, 64);
        assert_eq!(ring.first(), ring.last());
        assert_eq!(ring.len(), 65);
        for p in &ring {
            let d = haversine_m(c, *p);
            assert!((d - 1000.0).abs() < 0.01, "{d}");
        }
        assert!(point_within(c, std::slice::from_ref(&ring)));
        // A point 900 m away is inside, one 1,100 m away is not.
        assert!(point_within(
            destination(c, 33.0, 900.0),
            std::slice::from_ref(&ring)
        ));
        assert!(!point_intersects(destination(c, 33.0, 1100.0), &[ring]));
    }

    #[test]
    fn destination_moves_north_by_the_distance() {
        let to = destination((10.0, 20.0), 0.0, 111_195.0);
        assert!((to.0 - 10.0).abs() < 1e-9);
        assert!((to.1 - 21.0).abs() < 0.001, "{to:?}");
    }

    #[test]
    fn validity_finds_the_ring_problems() {
        assert_eq!(validity(&Shape::Point(1.0, 2.0)), Validity::Valid);
        assert!(matches!(
            validity(&Shape::Point(181.0, 2.0)),
            Validity::Invalid(_)
        ));
        assert_eq!(validity(&Shape::Polygon(vec![square()])), Validity::Valid);
        // A bow tie crosses itself.
        let bow: Ring = vec![
            (0.0, 0.0),
            (10.0, 10.0),
            (10.0, 0.0),
            (0.0, 10.0),
            (0.0, 0.0),
        ];
        assert_eq!(
            validity(&Shape::Polygon(vec![bow])).reason(),
            "Shell self-intersects"
        );
        let open: Ring = vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        assert_eq!(
            validity(&Shape::Polygon(vec![open])).reason(),
            "Shell is not closed"
        );
        let outside: Ring = Bbox::new(20.0, 20.0, 21.0, 21.0).ring();
        assert_eq!(
            validity(&Shape::Polygon(vec![square(), outside])).reason(),
            "Hole 1 lies outside the shell"
        );
        assert_eq!(Validity::Valid.reason(), "Valid Geometry");
    }

    #[test]
    fn polygon_text_keeps_its_holes() {
        let hole: Ring = Bbox::new(4.0, 4.0, 6.0, 6.0).ring();
        let wkt = Shape::Polygon(vec![square(), hole]).wkt();
        assert!(wkt.starts_with("POLYGON((0 0, 10 0"), "{wkt}");
        assert_eq!(wkt.matches('(').count(), 3);
    }
}
