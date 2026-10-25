//! Spatial analysis over the Explorer's features, computed in plain Rust.

use crate::data::mock::Overlay;
use crate::data::mock::spatial::features::all_features;
use crate::models::geo_ops::{Shape, haversine_m, point_intersects, point_within};
use crate::models::spatial::Feature;
use crate::models::spatial_analyze::{
    AnalysisRequest, AnalysisResult, Operation, Query, ResultColumn, ResultRow,
};
use crate::services::error::StudioError;

fn column(name: &str, numeric: bool) -> ResultColumn {
    ResultColumn {
        name: name.to_string(),
        numeric,
    }
}

fn point(f: &Feature) -> Option<(f64, f64)> {
    match f.shape {
        Shape::Point(x, y) => Some((x, y)),
        Shape::Polygon(_) => None,
    }
}

pub fn spatial_analyze(
    overlay: &Overlay,
    req: &AnalysisRequest,
) -> Result<AnalysisResult, StudioError> {
    let features = all_features(overlay, &req.source)?;
    let cap = req.limit as usize;
    match (&req.query, req.op) {
        (Query::Nearby { lon, lat, radius_m }, Operation::Nearby) => {
            let mut hits: Vec<(f64, &Feature, (f64, f64))> = features
                .iter()
                .filter_map(|f| {
                    let at = point(f)?;
                    let d = haversine_m((*lon, *lat), at);
                    (d <= *radius_m).then_some((d, f, at))
                })
                .collect();
            hits.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.id.cmp(&b.1.id)));
            let matching = hits.len() as u64;
            let rows = hits
                .into_iter()
                .take(cap)
                .map(|(d, f, at)| ResultRow {
                    id: f.id.clone(),
                    cells: vec![f.id.clone(), f.title.clone(), format!("{d:.1}")],
                    at,
                })
                .collect();
            Ok(AnalysisResult {
                request: req.clone(),
                columns: vec![
                    column(&req.id_field, false),
                    column(&req.label_field, false),
                    column("distance_m", true),
                ],
                rows,
                matching,
                measure_note: Some("Distance in metres · geodesic".to_string()),
            })
        }
        (Query::Area(rings), Operation::Within | Operation::Intersects) => {
            let test = |at: (f64, f64)| match req.op {
                Operation::Within => point_within(at, rings),
                _ => point_intersects(at, rings),
            };
            let mut hits: Vec<(&Feature, (f64, f64))> = features
                .iter()
                .filter_map(|f| {
                    let at = point(f)?;
                    test(at).then_some((f, at))
                })
                .collect();
            hits.sort_by(|a, b| a.0.id.cmp(&b.0.id));
            let matching = hits.len() as u64;
            let rows = hits
                .into_iter()
                .take(cap)
                .map(|(f, at)| ResultRow {
                    id: f.id.clone(),
                    cells: vec![
                        f.id.clone(),
                        f.title.clone(),
                        format!("{}", at.0),
                        format!("{}", at.1),
                    ],
                    at,
                })
                .collect();
            Ok(AnalysisResult {
                request: req.clone(),
                columns: vec![
                    column(&req.id_field, false),
                    column(&req.label_field, false),
                    column("lon", true),
                    column("lat", true),
                ],
                rows,
                matching,
                measure_note: None,
            })
        }
        _ => Err(StudioError::Rejected {
            reason: format!("{} cannot run on this connection.", req.op.label()),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::mock::spatial::spatial_setup;
    use crate::models::spatial::SourceRef;
    use crate::models::spatial_analyze::{AnalysisForm, Availability};

    fn request(form: AnalysisForm) -> AnalysisRequest {
        let o = Overlay::default();
        let setup = spatial_setup(&o);
        let source = setup
            .source(&SourceRef::new("places", "geometry"))
            .expect("source")
            .clone();
        form.check(&source, &Availability::Supported)
            .expect("valid")
    }

    #[test]
    fn nearby_ranks_by_distance_with_the_origin_first() {
        let r = spatial_analyze(&Overlay::default(), &request(AnalysisForm::default()))
            .expect("result");
        assert_eq!(r.rows[0].id, "place104", "the origin is its own nearest");
        assert_eq!(r.rows[0].cells[2], "0.0");
        let d: Vec<f64> = r
            .rows
            .iter()
            .map(|row| row.cells[2].parse().expect("number"))
            .collect();
        assert!(d.windows(2).all(|w| w[0] <= w[1]), "{d:?}");
        assert!(d.iter().all(|v| *v <= 1000.0));
        // Riverside Park is 118 m west and 56 m north of Cafe North Point.
        let park = r
            .rows
            .iter()
            .find(|row| row.id == "place103")
            .expect("park");
        let dist: f64 = park.cells[2].parse().expect("number");
        assert!((dist - 130.4).abs() < 5.0, "{dist}");
        assert!(r.rows.len() >= 4);
        assert_eq!(r.matching, r.rows.len() as u64);
        assert!(!r.limit_reached());
    }

    #[test]
    fn the_radius_is_a_distance_in_metres_not_degrees() {
        let small = spatial_analyze(&Overlay::default(), &request(AnalysisForm::default()))
            .expect("result");
        let big = spatial_analyze(
            &Overlay::default(),
            &request(AnalysisForm {
                radius: "1000".into(),
                unit: crate::models::spatial_analyze::DistanceUnit::Kilometres,
                ..AnalysisForm::default()
            }),
        )
        .expect("result");
        assert!(big.matching > small.matching);
        assert_eq!(big.matching, 104, "1,000 km reaches every place");
    }

    #[test]
    fn the_limit_cuts_rows_and_the_total_stays() {
        let r = spatial_analyze(
            &Overlay::default(),
            &request(AnalysisForm {
                limit: "2".into(),
                ..AnalysisForm::default()
            }),
        )
        .expect("result");
        assert_eq!(r.rows.len(), 2);
        assert!(r.matching > 2 && r.limit_reached());
    }

    #[test]
    fn within_a_rectangle_lists_the_points_inside_it() {
        let r = spatial_analyze(
            &Overlay::default(),
            &request(AnalysisForm {
                op: Operation::Within,
                ..AnalysisForm::default()
            }),
        )
        .expect("result");
        let ids: Vec<&str> = r.rows.iter().map(|row| row.id.as_str()).collect();
        // The default box is -73.995..-73.975, 40.742..40.756.
        for id in ["place104", "place103", "place101", "place102"] {
            assert!(ids.contains(&id), "{id} missing from {ids:?}");
        }
        assert!(
            !ids.contains(&"place098"),
            "the Brooklyn Bridge is far south"
        );
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
    }

    #[test]
    fn a_point_on_the_edge_intersects_but_is_not_within() {
        // A box whose west edge is exactly Cafe North Point's longitude.
        let form = AnalysisForm {
            west: "-73.9857".into(),
            east: "-73.97".into(),
            south: "40.74".into(),
            north: "40.76".into(),
            ..AnalysisForm::default()
        };
        let within = spatial_analyze(
            &Overlay::default(),
            &request(AnalysisForm {
                op: Operation::Within,
                ..form.clone()
            }),
        )
        .expect("result");
        let intersects = spatial_analyze(
            &Overlay::default(),
            &request(AnalysisForm {
                op: Operation::Intersects,
                ..form
            }),
        )
        .expect("result");
        assert!(!within.rows.iter().any(|r| r.id == "place104"));
        assert!(intersects.rows.iter().any(|r| r.id == "place104"));
    }

    #[test]
    fn an_empty_area_answers_with_no_rows() {
        let r = spatial_analyze(
            &Overlay::default(),
            &request(AnalysisForm {
                op: Operation::Within,
                west: "10".into(),
                east: "11".into(),
                south: "10".into(),
                north: "11".into(),
                ..AnalysisForm::default()
            }),
        )
        .expect("result");
        assert!(r.rows.is_empty() && r.matching == 0);
    }
}
