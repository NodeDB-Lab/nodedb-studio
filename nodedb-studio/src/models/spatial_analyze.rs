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
