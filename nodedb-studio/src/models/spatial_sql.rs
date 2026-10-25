//! Spatial analysis output: the SQL a checked request writes, the shape it
//! draws on the map, and the distance text.

use crate::models::geo_ops::{Ring, Shape};
use crate::models::spatial_analyze::{AnalysisRequest, Query};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::geo_ops::Bbox;
    use crate::models::spatial::{GeoSource, SourceRef};
    use crate::models::spatial_analyze::{
        AnalysisForm, AreaKind, Availability, DistanceUnit, Operation,
    };

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
}
