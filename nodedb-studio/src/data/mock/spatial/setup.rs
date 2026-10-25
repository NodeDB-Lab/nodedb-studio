//! What the Spatial screens discover: geometry sources, georeferenced
//! arrays, and the operations the connection runs.

use crate::data::mock::explorer::{all_records, array_schema, collection_schema, collections};
use crate::data::mock::{Overlay, spatial::features::shape_of};
use crate::models::collection::Profile;
use crate::models::geo_ops::Bbox;
use crate::models::geo_raster::{Affine, BandMeta};
use crate::models::schema::{FieldType, IndexKind};
use crate::models::spatial::{
    GeoSource, RasterDim, RasterSource, SliceAxis, SourceIndex, SourceRef, SpatialSetup,
};
use crate::models::spatial_analyze::{Availability, Operation, OperationSupport};

/// The GEOREF of `climate_cube`: the grid's pixel corners, so a cell centre
/// sits on the whole tenth of a degree the Explorer lists as its coordinate.
pub const CLIMATE_AFFINE: Affine = Affine::new([-92.05, 0.1, 0.0, 49.05, 0.0, -0.1]);

/// The marker a `temperature_c` cell holds where the sensor has no reading.
pub const NODATA: f64 = -9999.0;

fn unsupported(op: Operation, why: &str) -> OperationSupport {
    OperationSupport {
        op,
        availability: Availability::Unsupported(why.to_string()),
    }
}

fn operations() -> Vec<OperationSupport> {
    Operation::ALL
        .iter()
        .map(|op| match op {
            Operation::Nearby | Operation::Within | Operation::Intersects => OperationSupport {
                op: *op,
                availability: Availability::Supported,
            },
            Operation::Contains => unsupported(
                *op,
                "ST_Contains tests an area source. This connection's geometry sources hold points only.",
            ),
            Operation::Covers => unsupported(*op, "This connection does not report ST_Covers."),
            Operation::IsValid => unsupported(
                *op,
                "A validity scan of a whole source is not available. Check one feature in Map, Feature, Geometry.",
            ),
            Operation::Transform => unsupported(
                *op,
                "This connection reports no CRS transform between its sources' systems.",
            ),
            Operation::H3Bins => unsupported(*op, "This connection does not report H3 or geohash binning."),
        })
        .collect()
}

/// The geometry sources: every field of a collection that holds geometry.
fn sources(overlay: &Overlay) -> Vec<GeoSource> {
    collections()
        .into_iter()
        .filter(|c| c.profile == Profile::Spatial)
        .filter_map(|c| {
            let schema = collection_schema(&c.name)?;
            let (field, srid) = schema.geometry()?;
            debug_assert!(matches!(field.ty, FieldType::Geometry { .. }));
            let records = all_records(&c.name, overlay).unwrap_or_default();
            let shapes: Vec<_> = records
                .iter()
                .filter_map(|r| shape_of(r.get(&field.name)?))
                .collect();
            let extent = Bbox::from_points(
                shapes
                    .iter()
                    .filter_map(|s| s.bbox())
                    .flat_map(|b| [(b.west, b.south), (b.east, b.north)]),
            );
            let id_field = schema.id_field.clone().unwrap_or_else(|| "id".to_string());
            let label_field = schema
                .fields
                .iter()
                .find(|f| f.name == "name")
                .map_or_else(|| id_field.clone(), |f| f.name.clone());
            Some(GeoSource {
                source: SourceRef::new(&c.name, &field.name),
                id_field,
                label_field,
                srid: Some(srid),
                storage: c.profile.storage_label().to_string(),
                ordinates: "XY".to_string(),
                geometry_count: Some(shapes.len() as u64),
                record_count: Some(records.len() as u64),
                extent,
                indexes: schema
                    .indexes
                    .iter()
                    .filter(|i| i.kind == IndexKind::Spatial && i.fields.contains(&field.name))
                    .map(|i| SourceIndex {
                        name: i.name.clone(),
                        kind: i.kind.label().to_string(),
                    })
                    .collect(),
            })
        })
        .collect()
}

pub(super) fn rasters() -> Vec<RasterSource> {
    let Some(schema) = array_schema("climate_cube") else {
        return Vec::new();
    };
    let band = |name: &str, units: &str, nodata: Option<f64>| BandMeta {
        name: name.to_string(),
        dtype: "float32".to_string(),
        units: units.to_string(),
        nodata,
        scale: None,
        offset: None,
    };
    vec![RasterSource {
        array: schema.array.clone(),
        x_dim: "lon".to_string(),
        y_dim: "lat".to_string(),
        srid: 4326,
        affine: CLIMATE_AFFINE,
        rows: schema.dimension("lat").map_or(0, |d| d.points),
        cols: schema.dimension("lon").map_or(0, |d| d.points),
        bands: schema
            .attributes
            .iter()
            .map(|a| {
                let nodata = (a.name == "temperature_c").then_some(NODATA);
                band(&a.name, &a.unit, nodata)
            })
            .collect(),
        slice_axis: schema.dimension("time").map(|d| SliceAxis {
            name: d.name.clone(),
            points: d.points,
            origin: schema.time_origin,
            step: schema.time_step,
        }),
        dimensions: schema
            .dimensions
            .iter()
            .map(|d| RasterDim {
                name: d.name.clone(),
                start: d.start,
                end: d.end,
                points: d.points,
                unit: d.unit.clone(),
            })
            .collect(),
    }]
}

pub fn spatial_setup(overlay: &Overlay) -> SpatialSetup {
    SpatialSetup {
        sources: sources(overlay),
        rasters: rasters(),
        operations: operations(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_is_the_geometry_source() {
        let setup = spatial_setup(&Overlay::default());
        assert_eq!(setup.sources.len(), 1);
        let s = &setup.sources[0];
        assert_eq!(s.source.key(), "places.geometry");
        assert_eq!(s.srid, Some(4326));
        assert_eq!(s.geometry_count, Some(104));
        assert_eq!(s.indexes.len(), 1);
        assert_eq!(s.indexes[0].name, "idx_places_geometry");
        assert_eq!(s.label_field, "name");
        let e = s.extent.expect("extent");
        assert!(e.contains(-73.9857, 40.7484));
    }

    #[test]
    fn the_array_is_a_raster_not_a_source() {
        let setup = spatial_setup(&Overlay::default());
        assert!(
            setup
                .source(&SourceRef::new("climate_cube", "temperature_c"))
                .is_none()
        );
        let r = setup.raster("climate_cube").expect("raster");
        assert_eq!((r.rows, r.cols), (181, 360));
        assert_eq!(r.bands.len(), 2);
        assert_eq!(r.slice_axis.as_ref().map(|a| a.points), Some(24));
        assert!(r.affine.is_north_up());
    }

    #[test]
    fn the_georef_puts_cells_on_the_explorers_coordinates() {
        // The Explorer lists latitude 31.0 to 49.0 and longitude -92.0 to -56.1.
        let (lon, lat) = CLIMATE_AFFINE.centre(0, 0);
        assert!((lon + 92.0).abs() < 1e-9 && (lat - 49.0).abs() < 1e-9);
        let (lon, lat) = CLIMATE_AFFINE.centre(180, 359);
        assert!((lon + 56.1).abs() < 1e-9 && (lat - 31.0).abs() < 1e-9);
    }

    #[test]
    fn only_nearby_and_the_area_predicates_run() {
        let setup = spatial_setup(&Overlay::default());
        let ok: Vec<Operation> = setup
            .operations
            .iter()
            .filter(|o| o.availability == Availability::Supported)
            .map(|o| o.op)
            .collect();
        assert_eq!(
            ok,
            [Operation::Nearby, Operation::Within, Operation::Intersects]
        );
        assert_eq!(setup.operations.len(), Operation::ALL.len());
    }
}
