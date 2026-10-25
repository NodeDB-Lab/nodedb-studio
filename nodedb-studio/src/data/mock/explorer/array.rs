//! `climate_cube`: hourly temperature on a latitude and longitude grid.

use crate::data::mock::explorer::build::unit;
use crate::models::array_data::{ArraySchema, Attribute, Dimension, SliceRequest, SliceResult};
use crate::models::format::epoch;
use crate::services::error::StudioError;

/// Most cells one slice may return.
pub const MAX_CELLS: usize = 4096;

const LAT_MIN: f64 = 31.0;
const LAT_MAX: f64 = 49.0;
const LON_MIN: f64 = -92.0;
const LON_MAX: f64 = -56.1;

pub fn array_schema(array: &str) -> Option<ArraySchema> {
    if array != "climate_cube" {
        return None;
    }
    let dim = |name: &str, start: f64, end: f64, step: f64, points: u32, unit: &str| Dimension {
        name: name.to_string(),
        start,
        end,
        step,
        points,
        chunk: 1,
        unit: unit.to_string(),
    };
    let attr = |name: &str, unit: &str| Attribute {
        name: name.to_string(),
        dtype: "float32".to_string(),
        unit: unit.to_string(),
    };
    let mut lat = dim("lat", LAT_MIN, LAT_MAX, 0.1, 181, "degrees north");
    lat.chunk = 32;
    let mut lon = dim("lon", LON_MIN, LON_MAX, 0.1, 360, "degrees east");
    lon.chunk = 32;
    Some(ArraySchema {
        array: array.to_string(),
        dimensions: vec![dim("time", 0.0, 23.0, 1.0, 24, "hour (UTC)"), lat, lon],
        attributes: vec![attr("temperature_c", "°C"), attr("humidity_pct", "%")],
        time_origin: epoch(2026, 6, 13, 0, 0, 0),
        time_step: 3600,
    })
}

fn tenths(v: f64) -> i64 {
    (v * 10.0).round() as i64
}

/// The value at one cell. Smooth, and exactly 22.4 °C at 40.5 N, 73.5 W,
/// 12:00 UTC.
fn sample(attribute: &str, lat: f64, lon: f64, hour: u32) -> f64 {
    let (dl, dn) = (lat - 40.5, lon + 73.5);
    let dt = (f64::from(hour) - 12.0) / 12.0;
    let raw = match attribute {
        "humidity_pct" => 58.0 + 18.0 * (2.0 * dn).tanh() - 9.0 * (3.0 * dl).sin() - 6.0 * dt * dt,
        _ => {
            22.4 + 6.5 * (2.2 * dn).tanh()
                + 4.0 * (3.0 * dl).sin() * (0.6 + 0.4 * (2.0 * dn).cos())
                + 2.5 * (std::f64::consts::PI * dt).sin()
        }
    };
    let ripple = (unit(
        tenths(lat) as usize * 7919 + tenths(lon).unsigned_abs() as usize,
        51,
    ) - 0.5)
        * 0.4;
    // The ripple is zero at the reference cell so the sampled value stays exact.
    let ripple = if (dl, dn, hour) == (0.0, 0.0, 12) {
        0.0
    } else {
        ripple
    };
    ((raw + ripple) * 10.0).round() / 10.0
}

pub fn array_slice(array: &str, request: &SliceRequest) -> Result<SliceResult, StudioError> {
    let schema = array_schema(array).ok_or_else(|| StudioError::RecordMissing {
        collection: array.to_string(),
        id: String::new(),
    })?;
    let rejected = |reason: String| StudioError::Rejected { reason };
    if request.time_index >= 24 {
        return Err(rejected("The time axis has 24 steps, 0 to 23.".to_string()));
    }
    let attribute = schema
        .attributes
        .iter()
        .find(|a| a.name == request.attribute)
        .ok_or_else(|| {
            rejected(format!(
                "The array has no attribute `{}`.",
                request.attribute
            ))
        })?;
    for (name, (lo, hi), min, max) in [
        ("latitude", request.lat, LAT_MIN, LAT_MAX),
        ("longitude", request.lon, LON_MIN, LON_MAX),
    ] {
        if lo > hi {
            return Err(rejected(format!("The {name} range starts after it ends.")));
        }
        if lo < min - 1e-9 || hi > max + 1e-9 {
            return Err(rejected(format!(
                "The {name} range must stay within {min} to {max}."
            )));
        }
    }
    let (lat_lo, lat_hi) = (tenths(request.lat.0), tenths(request.lat.1));
    let (lon_lo, lon_hi) = (tenths(request.lon.0), tenths(request.lon.1));
    let cells = ((lat_hi - lat_lo + 1) * (lon_hi - lon_lo + 1)) as usize;
    if cells > MAX_CELLS {
        return Err(rejected(format!(
            "The slice has {cells} cells and the limit is {MAX_CELLS}. Narrow a range."
        )));
    }
    let lats: Vec<f64> = (lat_lo..=lat_hi).rev().map(|t| t as f64 / 10.0).collect();
    let lons: Vec<f64> = (lon_lo..=lon_hi).map(|t| t as f64 / 10.0).collect();
    let values: Vec<Vec<f64>> = lats
        .iter()
        .map(|lat| {
            lons.iter()
                .map(|lon| sample(&request.attribute, *lat, *lon, request.time_index))
                .collect()
        })
        .collect();
    let flat = values.iter().flatten().copied();
    let min = flat.clone().fold(f64::INFINITY, f64::min);
    let max = flat.fold(f64::NEG_INFINITY, f64::max);
    Ok(SliceResult {
        attribute: attribute.name.clone(),
        unit: attribute.unit.clone(),
        lats,
        lons,
        values,
        min,
        max,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> SliceRequest {
        SliceRequest {
            time_index: 12,
            lat: (40.0, 41.0),
            lon: (-74.0, -73.0),
            attribute: "temperature_c".into(),
        }
    }

    #[test]
    fn the_reference_slice_is_11_by_11_with_the_known_cell() {
        let r = array_slice("climate_cube", &request()).expect("slice");
        assert_eq!((r.lats.len(), r.lons.len()), (11, 11));
        // 40.5 N is row 5 from the top (north first); -73.5 is column 5.
        assert_eq!(r.lats[5], 40.5);
        assert_eq!(r.lons[5], -73.5);
        assert_eq!(r.cell(5, 5), Some(22.4));
    }

    #[test]
    fn shape_is_24_by_181_by_360() {
        assert_eq!(
            array_schema("climate_cube").map(|s| s.shape()),
            Some(vec![24, 181, 360])
        );
    }

    #[test]
    fn out_of_range_and_oversized_slices_are_rejected() {
        let mut r = request();
        r.lat = (30.0, 41.0);
        assert!(matches!(
            array_slice("climate_cube", &r),
            Err(StudioError::Rejected { .. })
        ));
        let mut r = request();
        r.lat = (31.0, 49.0);
        r.lon = (-92.0, -56.1);
        assert!(matches!(
            array_slice("climate_cube", &r),
            Err(StudioError::Rejected { .. })
        ));
        let mut r = request();
        r.lon = (-73.0, -74.0);
        assert!(matches!(
            array_slice("climate_cube", &r),
            Err(StudioError::Rejected { .. })
        ));
    }
}
