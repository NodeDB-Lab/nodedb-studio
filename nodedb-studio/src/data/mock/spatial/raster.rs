//! A band of `climate_cube` as a raster: raw values on the array's grid.

use std::rc::Rc;

use crate::data::mock::explorer::array_cell;
use crate::data::mock::spatial::setup::{CLIMATE_AFFINE, NODATA, rasters};
use crate::models::format::date_minute;
use crate::models::spatial::{RasterRequest, RasterSlice};
use crate::services::error::StudioError;

const ROWS: u32 = 181;
const COLS: u32 = 360;

/// Where the sensor has no reading: two patches at the grid's corners.
/// Only `temperature_c` has a NoData marker, so only it shows them.
fn uncovered(lat: f64, lon: f64) -> bool {
    let near = |clat: f64, clon: f64, r: f64| (lat - clat).hypot(lon - clon) < r;
    near(49.0, -56.1, 3.4) || near(31.0, -92.0, 2.5)
}

pub fn raster_slice(request: &RasterRequest) -> Result<RasterSlice, StudioError> {
    let source = rasters()
        .into_iter()
        .find(|r| r.array == request.array)
        .ok_or_else(|| StudioError::UnknownObject {
            kind: "raster",
            id: request.array.clone(),
        })?;
    let meta = source
        .bands
        .iter()
        .find(|b| b.name == request.band)
        .ok_or_else(|| StudioError::Rejected {
            reason: format!("The array has no attribute `{}`.", request.band),
        })?
        .clone();
    let axis = source
        .slice_axis
        .as_ref()
        .ok_or_else(|| StudioError::Rejected {
            reason: "The array has no dimension to slice.".to_string(),
        })?;
    if request.slice_index >= axis.points {
        return Err(StudioError::Rejected {
            reason: format!(
                "The {} axis has {} steps, 0 to {}.",
                axis.name,
                axis.points,
                axis.points - 1
            ),
        });
    }
    let mut raw = Vec::with_capacity((ROWS * COLS) as usize);
    for row in 0..ROWS {
        // Whole tenths of a degree, so a coordinate is exact.
        let lat = f64::from(490 - row as i32) / 10.0;
        for col in 0..COLS {
            let lon = f64::from(col as i32 - 920) / 10.0;
            let missing = meta.nodata.is_some() && uncovered(lat, lon);
            raw.push(if missing {
                NODATA
            } else {
                f64::from(array_cell(&request.band, lat, lon, request.slice_index) as f32)
            });
        }
    }
    Ok(RasterSlice {
        request: request.clone(),
        rows: ROWS,
        cols: COLS,
        srid: source.srid,
        affine: CLIMATE_AFFINE,
        meta,
        raw: Rc::from(raw),
        slice_label: format!("{} UTC", date_minute(axis.at(request.slice_index), 0)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(band: &str, hour: u32) -> RasterRequest {
        RasterRequest {
            array: "climate_cube".into(),
            slice_index: hour,
            band: band.into(),
        }
    }

    #[test]
    fn the_reference_cell_is_the_explorers_22_4_degrees() {
        let s = raster_slice(&req("temperature_c", 12)).expect("slice");
        assert_eq!((s.rows, s.cols), (181, 360));
        // 40.5 N, 73.5 W is row 85, column 185.
        assert_eq!(s.raw_at(85, 185), Some(f64::from(22.4f32)));
        assert_eq!(
            s.physical_at(85, 185).map(|v| (v * 10.0).round() / 10.0),
            Some(22.4)
        );
        assert_eq!(s.slice_label, "2026-06-13 12:00 UTC");
    }

    #[test]
    fn the_cells_agree_with_the_explorers_slices() {
        use crate::data::mock::explorer::array_slice;
        use crate::models::array_data::SliceRequest;
        let slice = array_slice(
            "climate_cube",
            &SliceRequest {
                time_index: 7,
                lat: (40.0, 41.0),
                lon: (-74.0, -73.0),
                attribute: "temperature_c".into(),
            },
        )
        .expect("slice");
        let s = raster_slice(&req("temperature_c", 7)).expect("raster");
        for (i, lat) in slice.lats.iter().enumerate() {
            for (j, lon) in slice.lons.iter().enumerate() {
                let (row, col) = CLIMATE_AFFINE
                    .cell_at(*lon, *lat, 181, 360)
                    .expect("inside the grid");
                let want = slice.values[i][j];
                let got = s.physical_at(row, col).expect("valid cell");
                assert!((got - want).abs() < 1e-4, "{lat} {lon}: {got} vs {want}");
            }
        }
    }

    #[test]
    fn nodata_cells_hold_the_marker_and_read_as_missing() {
        let s = raster_slice(&req("temperature_c", 12)).expect("slice");
        assert_eq!(s.raw_at(0, 359), Some(NODATA), "the north-east corner");
        assert_eq!(s.physical_at(0, 359), None);
        assert!(s.nodata_cells() > 0);
        let (lo, _) = s.range(true).expect("range");
        assert!(lo > -50.0, "the marker never reaches the color range: {lo}");
        let humidity = raster_slice(&req("humidity_pct", 12)).expect("slice");
        assert_eq!(humidity.nodata_cells(), 0, "no marker declared");
    }

    #[test]
    fn a_bad_band_or_hour_is_refused() {
        assert!(raster_slice(&req("pressure", 0)).is_err());
        assert!(raster_slice(&req("temperature_c", 24)).is_err());
        let bad = RasterRequest {
            array: "nope".into(),
            slice_index: 0,
            band: "x".into(),
        };
        assert!(matches!(
            raster_slice(&bad),
            Err(StudioError::UnknownObject { .. })
        ));
    }
}
