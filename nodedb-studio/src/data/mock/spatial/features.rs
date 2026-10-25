//! Geometry reads over the Explorer's records: a viewport, one feature, and
//! a validity check.

use nodedb_types::Value;
use nodedb_types::geometry::Geometry;

use crate::data::mock::Overlay;
use crate::data::mock::explorer::{all_records, collection_schema, plain_text};
use crate::models::geo_ops::{Shape, Validity, validity};
use crate::models::records::Record;
use crate::models::schema::{CollectionSchema, FieldType};
use crate::models::spatial::{
    Feature, FeatureRef, LayerPage, SourceRef, Viewport, ViewportRequest,
};
use crate::services::error::StudioError;

/// A stored geometry as a shape. A kind Studio does not draw is `None`.
pub(super) fn shape_of(value: &Value) -> Option<Shape> {
    match value {
        Value::Geometry(Geometry::Point { coordinates }) => {
            Some(Shape::Point(coordinates[0], coordinates[1]))
        }
        Value::Geometry(Geometry::Polygon { coordinates }) => Some(Shape::Polygon(
            coordinates
                .iter()
                .map(|ring| ring.iter().map(|c| (c[0], c[1])).collect())
                .collect(),
        )),
        _ => None,
    }
}

fn unknown_source(source: &SourceRef) -> StudioError {
    StudioError::UnknownObject {
        kind: "geometry source",
        id: source.key(),
    }
}

/// A collection's schema and records, when `source` names a geometry field.
fn read(
    overlay: &Overlay,
    source: &SourceRef,
) -> Result<(CollectionSchema, Vec<Record>), StudioError> {
    let schema = collection_schema(&source.collection).ok_or_else(|| unknown_source(source))?;
    let is_geometry = schema
        .field(&source.field)
        .is_some_and(|f| matches!(f.ty, FieldType::Geometry { .. }));
    if !is_geometry {
        return Err(unknown_source(source));
    }
    let records = all_records(&source.collection, overlay).ok_or_else(|| unknown_source(source))?;
    Ok((schema, records))
}

fn feature(schema: &CollectionSchema, source: &SourceRef, record: &Record) -> Option<Feature> {
    let shape = shape_of(record.get(&source.field)?)?;
    let title = schema
        .field("name")
        .and_then(|f| record.get(&f.name))
        .map_or_else(|| record.id.clone(), plain_text);
    let props = schema
        .fields
        .iter()
        .filter(|f| !matches!(f.ty, FieldType::Geometry { .. }))
        .filter_map(|f| Some((f.name.clone(), plain_text(record.get(&f.name)?))))
        .collect();
    Some(Feature {
        id: record.id.clone(),
        title,
        props,
        shape,
    })
}

/// Every feature of a source, in the collection's natural order.
pub(super) fn all_features(
    overlay: &Overlay,
    source: &SourceRef,
) -> Result<Vec<Feature>, StudioError> {
    let (schema, records) = read(overlay, source)?;
    Ok(records
        .iter()
        .filter_map(|r| feature(&schema, source, r))
        .collect())
}

pub fn spatial_viewport(
    overlay: &Overlay,
    request: &ViewportRequest,
) -> Result<Viewport, StudioError> {
    let mut layers = Vec::new();
    for source in &request.sources {
        let inside: Vec<Feature> = all_features(overlay, source)?
            .into_iter()
            .filter(|f| f.shape.touches(request.bbox))
            .collect();
        let matching = inside.len() as u64;
        layers.push(LayerPage {
            source: source.clone(),
            features: inside.into_iter().take(request.limit as usize).collect(),
            matching,
        });
    }
    Ok(Viewport {
        request: request.clone(),
        layers,
    })
}

pub fn spatial_feature(overlay: &Overlay, r: &FeatureRef) -> Result<Feature, StudioError> {
    all_features(overlay, &r.source)?
        .into_iter()
        .find(|f| f.id == r.id)
        .ok_or_else(|| StudioError::RecordMissing {
            collection: r.source.collection.clone(),
            id: r.id.clone(),
        })
}

pub fn spatial_validity(overlay: &Overlay, r: &FeatureRef) -> Result<Validity, StudioError> {
    Ok(validity(&spatial_feature(overlay, r)?.shape))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::geo_ops::Bbox;

    fn places() -> SourceRef {
        SourceRef::new("places", "geometry")
    }

    #[test]
    fn place104_is_the_explorers_point_in_lon_lat_order() {
        let f = spatial_feature(
            &Overlay::default(),
            &FeatureRef {
                source: places(),
                id: "place104".into(),
            },
        )
        .expect("feature");
        assert_eq!(f.title, "Cafe North Point");
        assert_eq!(f.shape, Shape::Point(-73.9857, 40.7484));
        assert!(
            f.props
                .contains(&("category".to_string(), "cafe".to_string()))
        );
        assert!(!f.props.iter().any(|(k, _)| k == "geometry"));
    }

    #[test]
    fn a_viewport_returns_only_what_the_box_holds_and_counts_the_rest() {
        let req = ViewportRequest {
            sources: vec![places()],
            bbox: Bbox::new(-74.0, 40.74, -73.97, 40.76),
            limit: 3,
        };
        let v = spatial_viewport(&Overlay::default(), &req).expect("viewport");
        let page = v.page(&places()).expect("page");
        assert_eq!(page.features.len(), 3);
        assert!(page.matching > 3 && page.capped());
        assert!(page.features.iter().all(|f| {
            f.shape
                .bbox()
                .is_some_and(|b| req.bbox.contains(b.west, b.south))
        }));
        let all = ViewportRequest { limit: 500, ..req };
        let v = spatial_viewport(&Overlay::default(), &all).expect("viewport");
        let page = v.page(&places()).expect("page");
        assert!(!page.capped());
        assert_eq!(page.matching, page.features.len() as u64);
    }

    #[test]
    fn a_box_over_the_ocean_holds_nothing() {
        let req = ViewportRequest {
            sources: vec![places()],
            bbox: Bbox::new(-40.0, 0.0, -39.0, 1.0),
            limit: 100,
        };
        let v = spatial_viewport(&Overlay::default(), &req).expect("viewport");
        assert_eq!(v.page(&places()).map(|p| p.matching), Some(0));
    }

    #[test]
    fn an_unknown_source_or_feature_is_an_error() {
        let o = Overlay::default();
        let bad = SourceRef::new("users", "geometry");
        let req = ViewportRequest {
            sources: vec![bad.clone()],
            bbox: Bbox::new(0.0, 0.0, 1.0, 1.0),
            limit: 1,
        };
        assert!(matches!(
            spatial_viewport(&o, &req),
            Err(StudioError::UnknownObject { .. })
        ));
        let missing = FeatureRef {
            source: places(),
            id: "place999".into(),
        };
        assert!(matches!(
            spatial_feature(&o, &missing),
            Err(StudioError::RecordMissing { .. })
        ));
    }

    #[test]
    fn a_point_feature_is_valid() {
        let r = FeatureRef {
            source: places(),
            id: "place104".into(),
        };
        assert_eq!(
            spatial_validity(&Overlay::default(), &r).expect("validity"),
            Validity::Valid
        );
    }
}
