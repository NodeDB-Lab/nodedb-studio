//! Spatial analysis: the typed draft, its checks, the SQL it makes, and the
//! result it answers with.
//!
//! The form and the SQL are one thing: the SQL is written from the checked
//! request, never typed, so the two cannot drift. Distances are metres,
//! converted from the unit a person chose before they reach the SQL.

use crate::models::geo_ops::{Ring, Shape, Validity, validity};
use crate::models::spatial::{GeoSource, SourceRef};

/// What an analysis does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Operation {
    /// Features within a distance of a point.
    #[default]
    Nearby,
    /// Features that lie inside an area.
    Within,
    /// Features that share any point with an area.
    Intersects,
    Contains,
    Covers,
    IsValid,
    Transform,
    H3Bins,
}

impl Operation {
    pub const ALL: [Operation; 8] = [
        Operation::Nearby,
        Operation::Within,
        Operation::Intersects,
        Operation::Contains,
        Operation::Covers,
        Operation::IsValid,
        Operation::Transform,
        Operation::H3Bins,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Operation::Nearby => "Nearby",
            Operation::Within => "Within area",
            Operation::Intersects => "Intersects area",
            Operation::Contains => "Contains",
            Operation::Covers => "Covers",
            Operation::IsValid => "Validity",
            Operation::Transform => "Transform CRS",
            Operation::H3Bins => "H3 bins",
        }
    }

    /// The function the operation runs.
    pub fn function(self) -> &'static str {
        match self {
            Operation::Nearby => "ST_DWithin",
            Operation::Within => "ST_Within",
            Operation::Intersects => "ST_Intersects",
            Operation::Contains => "ST_Contains",
            Operation::Covers => "ST_Covers",
            Operation::IsValid => "ST_IsValid",
            Operation::Transform => "ST_Transform",
            Operation::H3Bins => "H3",
        }
    }

    /// Whether the operation tests against an area.
    pub fn needs_area(self) -> bool {
        matches!(self, Operation::Within | Operation::Intersects)
    }

    /// One line on which operand is which.
    pub fn operands(self) -> &'static str {
        match self {
            Operation::Nearby => "Source geometry within the radius of the origin.",
            Operation::Within => {
                "ST_Within(source geometry, area): the source lies inside the area."
            }
            Operation::Intersects => "ST_Intersects(source geometry, area): they share a point.",
            Operation::Contains | Operation::Covers | Operation::IsValid => "",
            Operation::Transform | Operation::H3Bins => "",
        }
    }
}

/// Whether the connection can run an operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    Supported,
    /// Why it cannot, in words that name the missing capability.
    Unsupported(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationSupport {
    pub op: Operation,
    pub availability: Availability,
}

/// A length unit. A query always runs in metres.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DistanceUnit {
    #[default]
    Metres,
    Kilometres,
    Feet,
    Miles,
}

impl DistanceUnit {
    pub const ALL: [DistanceUnit; 4] = [
        DistanceUnit::Metres,
        DistanceUnit::Kilometres,
        DistanceUnit::Feet,
        DistanceUnit::Miles,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DistanceUnit::Metres => "m",
            DistanceUnit::Kilometres => "km",
            DistanceUnit::Feet => "ft",
            DistanceUnit::Miles => "mi",
        }
    }

    pub fn to_metres(self, value: f64) -> f64 {
        match self {
            DistanceUnit::Metres => value,
            DistanceUnit::Kilometres => value * 1000.0,
            DistanceUnit::Feet => value * 0.3048,
            DistanceUnit::Miles => value * 1609.344,
        }
    }
}

/// How an area is given.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AreaKind {
    #[default]
    Rectangle,
    /// A polygon drawn on the map.
    Polygon,
}

impl AreaKind {
    pub fn label(self) -> &'static str {
        match self {
            AreaKind::Rectangle => "Rectangle",
            AreaKind::Polygon => "Drawn polygon",
        }
    }
}

/// The geometry a query tests the source against.
#[derive(Debug, Clone, PartialEq)]
pub enum Query {
    Nearby {
        lon: f64,
        lat: f64,
        radius_m: f64,
    },
    /// An area as rings: shell first, then holes.
    Area(Vec<Ring>),
}

/// A checked analysis, ready to run.
#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisRequest {
    pub source: SourceRef,
    pub id_field: String,
    pub label_field: String,
    pub srid: u32,
    pub op: Operation,
    pub query: Query,
    pub limit: u32,
}

/// One input that failed its check, named so the form can mark it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub field: &'static str,
    pub message: String,
}

/// The most rows one analysis returns.
pub const LIMIT_MAX: u32 = 10_000;

/// The inputs as typed. Text until Run parses it, so a half-typed number
/// never reaches the server.
#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisForm {
    pub op: Operation,
    pub radius: String,
    pub unit: DistanceUnit,
    pub limit: String,
    pub lon: String,
    pub lat: String,
    pub area: AreaKind,
    pub west: String,
    pub south: String,
    pub east: String,
    pub north: String,
    /// The polygon the map drew, closed.
    pub polygon: Option<Ring>,
}

impl Default for AnalysisForm {
    /// Nearby `place104`, the Explorer's Cafe North Point, within 1,000 m.
    fn default() -> Self {
        Self {
            op: Operation::Nearby,
            radius: "1000".into(),
            unit: DistanceUnit::Metres,
            limit: "100".into(),
            lon: "-73.9857".into(),
            lat: "40.7484".into(),
            area: AreaKind::Rectangle,
            west: "-73.995".into(),
            south: "40.742".into(),
            east: "-73.975".into(),
            north: "40.756".into(),
            polygon: None,
        }
    }
}

fn number(label: &str, field: &'static str, text: &str, issues: &mut Vec<Issue>) -> Option<f64> {
    match text.trim().parse::<f64>() {
        Ok(v) if v.is_finite() => Some(v),
        Ok(_) => {
            issues.push(Issue {
                field,
                message: format!("{label} must be a finite number."),
            });
            None
        }
        Err(_) => {
            issues.push(Issue {
                field,
                message: format!("{label} must be a number."),
            });
            None
        }
    }
}

fn in_range(
    label: &str,
    field: &'static str,
    v: Option<f64>,
    (lo, hi): (f64, f64),
    issues: &mut Vec<Issue>,
) -> Option<f64> {
    let v = v?;
    if v < lo || v > hi {
        issues.push(Issue {
            field,
            message: format!("{label} must be between {lo} and {hi}."),
        });
        return None;
    }
    Some(v)
}

impl AnalysisForm {
    /// Parse and check the inputs against the source. Every failing input is
    /// named. The result is the request that runs and the SQL that shows.
    pub fn check(
        &self,
        source: &GeoSource,
        availability: &Availability,
    ) -> Result<AnalysisRequest, Vec<Issue>> {
        if let Availability::Unsupported(why) = availability {
            return Err(vec![Issue {
                field: "operation",
                message: why.clone(),
            }]);
        }
        let mut issues = Vec::new();
        let Some(srid) = source.srid else {
            return Err(vec![Issue {
                field: "source",
                message: format!(
                    "The SRID of {} is unknown. Set it in the Designer before analysing.",
                    source.source.key()
                ),
            }]);
        };
        let limit = match self.limit.trim().parse::<u32>() {
            Ok(n) if (1..=LIMIT_MAX).contains(&n) => Some(n),
            _ => {
                issues.push(Issue {
                    field: "limit",
                    message: format!("Limit must be a whole number from 1 to {LIMIT_MAX}."),
                });
                None
            }
        };
        let query = if self.op == Operation::Nearby {
            let lon = number("Longitude", "lon", &self.lon, &mut issues);
            let lat = number("Latitude", "lat", &self.lat, &mut issues);
            let lon = in_range("Longitude", "lon", lon, (-180.0, 180.0), &mut issues);
            let lat = in_range("Latitude", "lat", lat, (-90.0, 90.0), &mut issues);
            let radius = number("Radius", "radius", &self.radius, &mut issues);
            let radius = radius.and_then(|r| {
                if r <= 0.0 {
                    issues.push(Issue {
                        field: "radius",
                        message: "Radius must be greater than 0.".to_string(),
                    });
                    None
                } else {
                    Some(self.unit.to_metres(r))
                }
            });
            match (lon, lat, radius) {
                (Some(lon), Some(lat), Some(radius_m)) => {
                    Some(Query::Nearby { lon, lat, radius_m })
                }
                _ => None,
            }
        } else if self.op.needs_area() {
            self.area_rings(&mut issues).map(Query::Area)
        } else {
            issues.push(Issue {
                field: "operation",
                message: format!("{} has no form yet.", self.op.label()),
            });
            None
        };
        match (query, limit) {
            (Some(query), Some(limit)) if issues.is_empty() => Ok(AnalysisRequest {
                source: source.source.clone(),
                id_field: source.id_field.clone(),
                label_field: source.label_field.clone(),
                srid,
                op: self.op,
                query,
                limit,
            }),
            _ => Err(issues),
        }
    }

    fn area_rings(&self, issues: &mut Vec<Issue>) -> Option<Vec<Ring>> {
        match self.area {
            AreaKind::Rectangle => {
                let w = number("West", "west", &self.west, issues);
                let s = number("South", "south", &self.south, issues);
                let e = number("East", "east", &self.east, issues);
                let n = number("North", "north", &self.north, issues);
                let w = in_range("West", "west", w, (-180.0, 180.0), issues);
                let e = in_range("East", "east", e, (-180.0, 180.0), issues);
                let s = in_range("South", "south", s, (-90.0, 90.0), issues);
                let n = in_range("North", "north", n, (-90.0, 90.0), issues);
                let (w, s, e, n) = (w?, s?, e?, n?);
                if w >= e {
                    issues.push(Issue {
                        field: "west",
                        message: "West must be less than east. A box across the antimeridian needs two queries."
                            .to_string(),
                    });
                    return None;
                }
                if s >= n {
                    issues.push(Issue {
                        field: "south",
                        message: "South must be less than north.".to_string(),
                    });
                    return None;
                }
                Some(vec![crate::models::geo_ops::Bbox::new(w, s, e, n).ring()])
            }
            AreaKind::Polygon => {
                let Some(ring) = self.polygon.clone() else {
                    issues.push(Issue {
                        field: "area",
                        message: "Draw a polygon on the map, then use it here.".to_string(),
                    });
                    return None;
                };
                let shape = Shape::Polygon(vec![ring.clone()]);
                if let Validity::Invalid(why) = validity(&shape) {
                    issues.push(Issue {
                        field: "area",
                        message: format!("The drawn polygon is not valid: {why}."),
                    });
                    return None;
                }
                Some(vec![ring])
            }
        }
    }
}

/// The text of one coordinate in SQL.
fn n(v: f64) -> String {
    format!("{v}")
}

impl AnalysisRequest {
    /// `ST_Point(-73.9857, 40.7484)` or the area as a geometry expression.
    fn area_sql(rings: &[Ring], srid: u32) -> String {
        if let [ring] = rings
            && ring.len() == 5
            && let Some(b) = Shape::Polygon(vec![ring.clone()]).bbox()
            && ring == &b.ring()
        {
            return format!(
                "ST_MakeEnvelope({}, {}, {}, {})",
                n(b.west),
                n(b.south),
                n(b.east),
                n(b.north)
            );
        }
        format!(
            "ST_GeomFromText('{}', {})",
            Shape::Polygon(rings.to_vec()).wkt(),
            srid
        )
    }

    /// The complete statement. Longitude comes before latitude in every
    /// point, and the radius is metres.
    pub fn sql(&self) -> String {
        let (id, label, from) = (&self.id_field, &self.label_field, &self.source.collection);
        let geom = &self.source.field;
        let limit = self.limit;
        match &self.query {
            Query::Nearby { lon, lat, radius_m } => {
                let point = format!("ST_Point({}, {})", n(*lon), n(*lat));
                format!(
                    "SELECT {id}, {label},\n  ST_Distance({geom},\n    {point}\n  ) AS distance_m\nFROM {from}\nWHERE ST_DWithin(\n  {geom},\n  {point},\n  {}\n)\nORDER BY distance_m\nLIMIT {limit};",
                    n(*radius_m)
                )
            }
            Query::Area(rings) => format!(
                "SELECT {id}, {label},\n  ST_X({geom}) AS lon,\n  ST_Y({geom}) AS lat\nFROM {from}\nWHERE {}(\n  {geom},\n  {}\n)\nORDER BY {id}\nLIMIT {limit};",
                self.op.function(),
                Self::area_sql(rings, self.srid)
            ),
        }
    }

    /// The query geometry as a shape, for the map: the radius as a ring, or
    /// the area.
    pub fn query_shape(&self) -> Shape {
        match &self.query {
            Query::Nearby { lon, lat, radius_m } => {
                Shape::Polygon(vec![crate::models::geo_ops::buffer_ring(
                    (*lon, *lat),
                    *radius_m,
                    96,
                )])
            }
            Query::Area(rings) => Shape::Polygon(rings.clone()),
        }
    }

    /// A short name for a layer or a tab.
    pub fn title(&self) -> String {
        match &self.query {
            Query::Nearby { radius_m, .. } => {
                format!("{} within {}", self.op.label(), distance_text(*radius_m))
            }
            Query::Area(_) => self.op.label().to_string(),
        }
    }
}

/// `1,000 m` or `2.5 km`.
pub fn distance_text(metres: f64) -> String {
    if metres >= 10_000.0 {
        format!(
            "{} km",
            crate::models::format::thousands((metres / 1000.0).round() as u64)
        )
    } else if metres >= 1000.0 && (metres / 1000.0).fract() != 0.0 {
        format!("{} km", (metres / 100.0).round() / 10.0)
    } else if metres >= 1000.0 {
        format!(
            "{} m",
            crate::models::format::thousands(metres.round() as u64)
        )
    } else {
        format!("{} m", (metres * 10.0).round() / 10.0)
    }
}

/// One column of a result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultColumn {
    pub name: String,
    /// Numbers align right.
    pub numeric: bool,
}

/// One row of a result.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultRow {
    pub id: String,
    pub cells: Vec<String>,
    /// Where the feature is, `(lon, lat)`, for Show on map.
    pub at: (f64, f64),
}

/// What an analysis answered.
#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisResult {
    /// The request the rows answer. A changed form is compared against it.
    pub request: AnalysisRequest,
    pub columns: Vec<ResultColumn>,
    pub rows: Vec<ResultRow>,
    /// How many features matched, before the limit.
    pub matching: u64,
    /// The unit line under the header, when the result has a measure.
    pub measure_note: Option<String>,
}

impl AnalysisResult {
    /// Whether the limit cut matches.
    pub fn limit_reached(&self) -> bool {
        self.matching > self.rows.len() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::geo_ops::Bbox;

    fn source() -> GeoSource {
        GeoSource {
            source: SourceRef::new("places", "geometry"),
            id_field: "feature_id".into(),
            label_field: "name".into(),
            srid: Some(4326),
            storage: "Columnar / Spatial".into(),
            ordinates: "XY".into(),
            geometry_count: Some(104),
            record_count: Some(104),
            extent: None,
            indexes: Vec::new(),
        }
    }

    fn ok() -> Availability {
        Availability::Supported
    }

    #[test]
    fn nearby_writes_longitude_before_latitude_and_metres() {
        let req = AnalysisForm::default()
            .check(&source(), &ok())
            .expect("valid");
        let sql = req.sql();
        assert_eq!(
            sql,
            "SELECT feature_id, name,\n  ST_Distance(geometry,\n    ST_Point(-73.9857, 40.7484)\n  ) AS distance_m\nFROM places\nWHERE ST_DWithin(\n  geometry,\n  ST_Point(-73.9857, 40.7484),\n  1000\n)\nORDER BY distance_m\nLIMIT 100;"
        );
        assert_eq!(sql.lines().count(), 12);
    }

    #[test]
    fn the_radius_converts_to_metres_before_it_reaches_the_sql() {
        let form = AnalysisForm {
            radius: "2".into(),
            unit: DistanceUnit::Kilometres,
            ..AnalysisForm::default()
        };
        let req = form.check(&source(), &ok()).expect("valid");
        assert_eq!(
            req.query,
            Query::Nearby {
                lon: -73.9857,
                lat: 40.7484,
                radius_m: 2000.0
            }
        );
        assert!(req.sql().contains("  2000\n)"));
        assert_eq!(DistanceUnit::Feet.to_metres(1000.0), 304.8);
        assert_eq!(DistanceUnit::Miles.to_metres(1.0), 1609.344);
    }

    #[test]
    fn every_bad_input_is_named() {
        let form = AnalysisForm {
            radius: "-5".into(),
            limit: "0".into(),
            lon: "190".into(),
            lat: "abc".into(),
            ..AnalysisForm::default()
        };
        let issues = form.check(&source(), &ok()).expect_err("invalid");
        let fields: Vec<&str> = issues.iter().map(|i| i.field).collect();
        for f in ["lon", "lat", "radius", "limit"] {
            assert!(fields.contains(&f), "{f} missing from {fields:?}");
        }
        let nan = AnalysisForm {
            lon: "NaN".into(),
            ..AnalysisForm::default()
        };
        assert!(nan.check(&source(), &ok()).is_err());
        let huge = AnalysisForm {
            limit: "10001".into(),
            ..AnalysisForm::default()
        };
        assert!(huge.check(&source(), &ok()).is_err());
    }

    #[test]
    fn an_unknown_srid_needs_resolving_first() {
        let s = GeoSource {
            srid: None,
            ..source()
        };
        let issues = AnalysisForm::default()
            .check(&s, &ok())
            .expect_err("unknown");
        assert_eq!(issues[0].field, "source");
        assert!(issues[0].message.contains("unknown"));
    }

    #[test]
    fn an_unsupported_operation_keeps_the_draft_and_says_why() {
        let form = AnalysisForm {
            op: Operation::Contains,
            ..AnalysisForm::default()
        };
        let why = Availability::Unsupported("ST_Contains is not reported.".into());
        let issues = form.check(&source(), &why).expect_err("unsupported");
        assert_eq!(issues[0].message, "ST_Contains is not reported.");
    }

    #[test]
    fn a_rectangle_area_writes_an_envelope() {
        let form = AnalysisForm {
            op: Operation::Within,
            ..AnalysisForm::default()
        };
        let req = form.check(&source(), &ok()).expect("valid");
        let sql = req.sql();
        assert!(sql.contains("WHERE ST_Within(\n  geometry,\n  ST_MakeEnvelope(-73.995, 40.742, -73.975, 40.756)\n)"), "{sql}");
        assert!(sql.contains("ST_X(geometry) AS lon"));
        let req = AnalysisForm {
            op: Operation::Intersects,
            ..form
        }
        .check(&source(), &ok())
        .expect("valid");
        assert!(req.sql().contains("ST_Intersects("));
    }

    #[test]
    fn a_reversed_rectangle_is_refused() {
        let form = AnalysisForm {
            op: Operation::Within,
            west: "-73.9".into(),
            east: "-74.0".into(),
            ..AnalysisForm::default()
        };
        let issues = form.check(&source(), &ok()).expect_err("reversed");
        assert_eq!(issues[0].field, "west");
    }

    #[test]
    fn a_drawn_polygon_writes_wkt_and_must_be_valid() {
        let ring: Ring = vec![
            (-73.99, 40.74),
            (-73.97, 40.74),
            (-73.98, 40.76),
            (-73.99, 40.74),
        ];
        let form = AnalysisForm {
            op: Operation::Within,
            area: AreaKind::Polygon,
            polygon: Some(ring),
            ..AnalysisForm::default()
        };
        let sql = form.check(&source(), &ok()).expect("valid").sql();
        assert!(sql.contains("ST_GeomFromText('POLYGON((-73.99 40.74, -73.97 40.74, -73.98 40.76, -73.99 40.74))', 4326)"), "{sql}");
        let bow: Ring = vec![(0.0, 0.0), (1.0, 1.0), (1.0, 0.0), (0.0, 1.0), (0.0, 0.0)];
        let bad = AnalysisForm {
            polygon: Some(bow),
            ..form.clone()
        };
        assert!(bad.check(&source(), &ok()).is_err());
        let none = AnalysisForm {
            polygon: None,
            ..form
        };
        assert_eq!(
            none.check(&source(), &ok()).expect_err("none")[0].field,
            "area"
        );
    }

    #[test]
    fn the_query_shape_is_the_radius_or_the_area() {
        let req = AnalysisForm::default()
            .check(&source(), &ok())
            .expect("valid");
        let Shape::Polygon(rings) = req.query_shape() else {
            panic!("a ring");
        };
        assert_eq!(rings[0].len(), 97);
        let area = AnalysisForm {
            op: Operation::Within,
            ..AnalysisForm::default()
        }
        .check(&source(), &ok())
        .expect("valid");
        assert_eq!(
            area.query_shape().bbox(),
            Some(Bbox::new(-73.995, 40.742, -73.975, 40.756))
        );
    }

    #[test]
    fn titles_and_distances_read_as_words() {
        let req = AnalysisForm::default()
            .check(&source(), &ok())
            .expect("valid");
        assert_eq!(req.title(), "Nearby within 1,000 m");
        assert_eq!(distance_text(243.8), "243.8 m");
        assert_eq!(distance_text(2500.0), "2.5 km");
        assert_eq!(distance_text(25_000.0), "25 km");
    }

    #[test]
    fn the_limit_reached_counts_matches_beyond_the_rows() {
        let req = AnalysisForm::default()
            .check(&source(), &ok())
            .expect("valid");
        let result = AnalysisResult {
            request: req,
            columns: Vec::new(),
            rows: Vec::new(),
            matching: 4,
            measure_note: None,
        };
        assert!(result.limit_reached());
    }
}
