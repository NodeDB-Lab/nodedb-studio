//! Text the Spatial screens show: CRS units, values, and the SQL that Open
//! in Query carries.

use crate::models::geo_ops::Bbox;
use crate::models::geo_raster::{BandMeta, ValueMode};
use crate::models::spatial::{GeoSource, RasterSource};

/// The unit a CRS measures in, when Studio knows it. An SRID it does not
/// know has no unit, and no unit is guessed.
pub fn crs_unit(srid: u32) -> Option<&'static str> {
    match srid {
        4326 | 4269 | 4258 => Some("degrees"),
        3857 | 3395 => Some("metres"),
        // UTM zones, north and south, on WGS 84 and NAD83.
        32601..=32660 | 32701..=32760 | 26901..=26923 => Some("metres"),
        _ => None,
    }
}

/// The short unit mark for a pixel size: `°` or ` m`.
pub fn unit_mark(srid: u32) -> Option<&'static str> {
    match crs_unit(srid)? {
        "degrees" => Some("°"),
        _ => Some(" m"),
    }
}

/// A number with at most `places` decimals and no trailing zeros.
pub fn trimmed(v: f64, places: usize) -> String {
    let s = format!("{v:.places$}");
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    };
    if s == "-0" { "0".to_string() } else { s }
}

/// A raw cell value as its type writes it. A `float32` prints at `f32`
/// width, so 22.4 reads `22.4` and not `22.399999618530273`.
pub fn raw_text(meta: &BandMeta, raw: f64) -> String {
    if meta.dtype.eq_ignore_ascii_case("float32") {
        format!("{}", raw as f32)
    } else if raw.fract() == 0.0 && raw.abs() < 1e15 {
        let t = crate::models::format::thousands(raw.abs() as u64);
        if raw < 0.0 { format!("-{t}") } else { t }
    } else {
        trimmed(raw, 6)
    }
}

/// A physical value at five decimals, the precision a scaled integer holds.
pub fn physical_text(v: f64) -> String {
    trimmed(v, 5)
}

/// A scale or offset as the catalog states it.
pub fn factor_text(v: f64) -> String {
    let s = format!("{v}");
    if s.len() > 12 { format!("{v:e}") } else { s }
}

/// The SQL of the features that touch a box.
pub fn viewport_sql(source: &GeoSource, bbox: Bbox, limit: u32) -> String {
    let r = |v: f64| trimmed(v, 6);
    let (id, label) = (&source.id_field, &source.label_field);
    let (geom, from) = (&source.source.field, &source.source.collection);
    format!(
        "SELECT {id}, {label}, ST_AsGeoJSON({geom}) AS {geom}\nFROM {from}\nWHERE ST_Intersects(\n  {geom},\n  ST_MakeEnvelope({}, {}, {}, {})\n)\nLIMIT {limit};",
        r(bbox.west),
        r(bbox.south),
        r(bbox.east),
        r(bbox.north)
    )
}

/// The SQL of one band at one slice of a raster.
pub fn raster_sql(source: &RasterSource, slice_index: u32, band: &str, mode: ValueMode) -> String {
    let axis = source
        .slice_axis
        .as_ref()
        .map_or_else(|| "time".to_string(), |a| a.name.clone());
    let physical = if mode == ValueMode::Physical {
        ", physical => true"
    } else {
        ""
    };
    format!(
        "SELECT * FROM ARRAY_SLICE(\n  '{}',\n  '{{{axis}: [{slice_index}, {slice_index}]}}',\n  ['{band}']{physical}\n);",
        source.array
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::geo_raster::Affine;
    use crate::models::spatial::{SliceAxis, SourceRef};

    fn band(dtype: &str) -> BandMeta {
        BandMeta {
            name: "x".into(),
            dtype: dtype.into(),
            units: String::new(),
            nodata: None,
            scale: None,
            offset: None,
        }
    }

    #[test]
    fn a_crs_unit_is_known_or_absent() {
        assert_eq!(crs_unit(4326), Some("degrees"));
        assert_eq!(crs_unit(3857), Some("metres"));
        assert_eq!(crs_unit(32647), Some("metres"));
        assert_eq!(crs_unit(99999), None, "no guessed metres");
        assert_eq!(unit_mark(4326), Some("°"));
        assert_eq!(unit_mark(32647), Some(" m"));
        assert_eq!(unit_mark(99999), None);
    }

    #[test]
    fn numbers_trim_without_losing_digits() {
        assert_eq!(trimmed(0.11999, 5), "0.11999");
        assert_eq!(trimmed(22.399_999_618_530_27, 5), "22.4");
        assert_eq!(trimmed(30.0, 5), "30");
        assert_eq!(trimmed(-0.000_001, 5), "0");
        assert_eq!(physical_text(0.119_990_000_1), "0.11999");
    }

    #[test]
    fn raw_values_print_at_their_type_width() {
        assert_eq!(raw_text(&band("float32"), f64::from(22.4f32)), "22.4");
        assert_eq!(raw_text(&band("uint16"), 11_636.0), "11,636");
        assert_eq!(raw_text(&band("int16"), -32_768.0), "-32,768");
        assert_eq!(raw_text(&band("float64"), 0.25), "0.25");
    }

    #[test]
    fn factors_stay_readable() {
        assert_eq!(factor_text(0.0000275), "0.0000275");
        assert_eq!(factor_text(-0.2), "-0.2");
        assert_eq!(factor_text(1.0), "1");
    }

    #[test]
    fn the_viewport_sql_is_an_envelope_over_the_geometry_field() {
        let source = GeoSource {
            source: SourceRef::new("places", "geometry"),
            id_field: "feature_id".into(),
            label_field: "name".into(),
            srid: Some(4326),
            storage: String::new(),
            ordinates: "XY".into(),
            geometry_count: None,
            record_count: None,
            extent: None,
            indexes: Vec::new(),
        };
        let sql = viewport_sql(&source, Bbox::new(-74.02, 40.69, -73.9, 40.8), 500);
        assert!(
            sql.contains("ST_MakeEnvelope(-74.02, 40.69, -73.9, 40.8)"),
            "{sql}"
        );
        assert!(sql.starts_with(
            "SELECT feature_id, name, ST_AsGeoJSON(geometry) AS geometry\nFROM places"
        ));
        assert!(sql.ends_with("LIMIT 500;"));
    }

    #[test]
    fn the_raster_sql_slices_the_axis_and_reads_physically() {
        let source = RasterSource {
            array: "climate_cube".into(),
            x_dim: "lon".into(),
            y_dim: "lat".into(),
            srid: 4326,
            affine: Affine::new([-92.05, 0.1, 0.0, 49.05, 0.0, -0.1]),
            rows: 181,
            cols: 360,
            bands: Vec::new(),
            slice_axis: Some(SliceAxis {
                name: "time".into(),
                points: 24,
                origin: 0,
                step: 3600,
            }),
            dimensions: Vec::new(),
        };
        let sql = raster_sql(&source, 12, "temperature_c", ValueMode::Physical);
        assert_eq!(
            sql,
            "SELECT * FROM ARRAY_SLICE(\n  'climate_cube',\n  '{time: [12, 12]}',\n  ['temperature_c'], physical => true\n);"
        );
        assert!(!raster_sql(&source, 3, "humidity_pct", ValueMode::Raw).contains("physical"));
    }
}
