//! The Spatial domain: geometry sources, features, viewport reads, and
//! georeferenced rasters.
//!
//! A layer is a way to draw a source, not a storage engine. A source is a
//! geometry field of a collection. An array with a GEOREF is a raster, not a
//! collection of point features, so the two are listed apart.

use std::rc::Rc;

use crate::models::geo_ops::{Bbox, Shape};
use crate::models::geo_raster::{Affine, BandMeta};
use crate::models::spatial_analyze::OperationSupport;

/// One geometry field of one collection.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceRef {
    pub collection: String,
    pub field: String,
}

impl SourceRef {
    pub fn new(collection: &str, field: &str) -> Self {
        Self {
            collection: collection.to_string(),
            field: field.to_string(),
        }
    }

    /// `places.geometry`.
    pub fn key(&self) -> String {
        format!("{}.{}", self.collection, self.field)
    }
}

/// A feature: one record of a source.
#[derive(Debug, Clone, PartialEq)]
pub struct FeatureRef {
    pub source: SourceRef,
    pub id: String,
}

/// An index the catalog lists on a geometry field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceIndex {
    pub name: String,
    pub kind: String,
}

/// What the catalog says about a geometry field. A value the catalog does
/// not report is `None` and reads as Unknown.
#[derive(Debug, Clone, PartialEq)]
pub struct GeoSource {
    pub source: SourceRef,
    /// The field that identifies a record.
    pub id_field: String,
    /// The field that names a record.
    pub label_field: String,
    /// The SRID the field declares. `None` needs an explicit resolution.
    pub srid: Option<u32>,
    /// The collection's storage profile, such as `Columnar / Spatial`.
    pub storage: String,
    /// The ordinates a geometry carries: `XY`, `XYZ`, `XYM`, `XYZM`.
    pub ordinates: String,
    /// Records with a geometry, apart from records.
    pub geometry_count: Option<u64>,
    pub record_count: Option<u64>,
    pub extent: Option<Bbox>,
    pub indexes: Vec<SourceIndex>,
}

/// A georeferenced array.
#[derive(Debug, Clone, PartialEq)]
pub struct RasterSource {
    pub array: String,
    /// The GEOREF's axes: the dimension that runs along a row and the one
    /// that runs down a column.
    pub x_dim: String,
    pub y_dim: String,
    pub srid: u32,
    pub affine: Affine,
    pub rows: u32,
    pub cols: u32,
    pub bands: Vec<BandMeta>,
    /// The non-spatial dimension a slice fixes.
    pub slice_axis: Option<SliceAxis>,
    pub dimensions: Vec<RasterDim>,
}

/// A dimension of the array, as the catalog lists it.
#[derive(Debug, Clone, PartialEq)]
pub struct RasterDim {
    pub name: String,
    pub start: f64,
    pub end: f64,
    pub points: u32,
    pub unit: String,
}

/// The coordinate axis a slice fixes, such as time.
#[derive(Debug, Clone, PartialEq)]
pub struct SliceAxis {
    pub name: String,
    pub points: u32,
    /// Seconds since the epoch at index 0, and the step in seconds.
    pub origin: i64,
    pub step: i64,
}

impl SliceAxis {
    pub fn at(&self, index: u32) -> i64 {
        self.origin + i64::from(index) * self.step
    }
}

/// Everything the Spatial screens discover on a connection.
#[derive(Debug, Clone, PartialEq)]
pub struct SpatialSetup {
    pub sources: Vec<GeoSource>,
    pub rasters: Vec<RasterSource>,
    pub operations: Vec<OperationSupport>,
}

impl SpatialSetup {
    pub fn source(&self, source: &SourceRef) -> Option<&GeoSource> {
        self.sources.iter().find(|s| s.source == *source)
    }

    pub fn raster(&self, array: &str) -> Option<&RasterSource> {
        self.rasters.iter().find(|r| r.array == array)
    }
}

/// A feature with the values a map and an inspector show.
#[derive(Debug, Clone, PartialEq)]
pub struct Feature {
    pub id: String,
    pub title: String,
    /// Every non-geometry field, as text, in schema order.
    pub props: Vec<(String, String)>,
    pub shape: Shape,
}

/// A bounded read of the features inside a box.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewportRequest {
    pub sources: Vec<SourceRef>,
    pub bbox: Bbox,
    /// The most features one source returns.
    pub limit: u32,
}

/// One source's part of a viewport.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerPage {
    pub source: SourceRef,
    pub features: Vec<Feature>,
    /// How many features the box holds, before the limit.
    pub matching: u64,
}

impl LayerPage {
    /// Whether the limit cut features the box holds.
    pub fn capped(&self) -> bool {
        self.matching > self.features.len() as u64
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Viewport {
    pub request: ViewportRequest,
    pub layers: Vec<LayerPage>,
}

impl Viewport {
    pub fn page(&self, source: &SourceRef) -> Option<&LayerPage> {
        self.layers.iter().find(|l| l.source == *source)
    }

    pub fn feature(&self, r: &FeatureRef) -> Option<&Feature> {
        self.page(&r.source)?.features.iter().find(|f| f.id == r.id)
    }
}

/// What a raster read asks for: one band at one coordinate of the slice axis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterRequest {
    pub array: String,
    pub slice_index: u32,
    pub band: String,
}

/// One band over the whole grid at one slice, with raw values.
#[derive(Debug, Clone, PartialEq)]
pub struct RasterSlice {
    pub request: RasterRequest,
    pub rows: u32,
    pub cols: u32,
    pub srid: u32,
    pub affine: Affine,
    pub meta: BandMeta,
    /// `rows * cols` raw values, row by row. NoData cells hold the marker.
    pub raw: Rc<[f64]>,
    /// The slice coordinate, as text.
    pub slice_label: String,
}

impl RasterSlice {
    pub fn raw_at(&self, row: u32, col: u32) -> Option<f64> {
        if row >= self.rows || col >= self.cols {
            return None;
        }
        self.raw
            .get(row as usize * self.cols as usize + col as usize)
            .copied()
    }

    /// The physical value of a valid cell. `None` for NoData.
    pub fn physical_at(&self, row: u32, col: u32) -> Option<f64> {
        self.meta.physical(self.raw_at(row, col)?)
    }

    /// The least and greatest of the valid cells, raw or physical.
    pub fn range(&self, physical: bool) -> Option<(f64, f64)> {
        let mut out: Option<(f64, f64)> = None;
        for raw in self.raw.iter() {
            let Some(p) = self.meta.physical(*raw) else {
                continue;
            };
            let v = if physical { p } else { *raw };
            out = Some(match out {
                Some((lo, hi)) => (lo.min(v), hi.max(v)),
                None => (v, v),
            });
        }
        out
    }

    pub fn nodata_cells(&self) -> usize {
        self.raw.iter().filter(|v| self.meta.is_nodata(**v)).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slice(raw: Vec<f64>) -> RasterSlice {
        RasterSlice {
            request: RasterRequest {
                array: "scene".into(),
                slice_index: 0,
                band: "red".into(),
            },
            rows: 2,
            cols: 2,
            srid: 32647,
            affine: Affine::new([800_000.0, 30.0, 0.0, 360_000.0, 0.0, -30.0]),
            meta: BandMeta {
                name: "red".into(),
                dtype: "uint16".into(),
                units: "reflectance".into(),
                nodata: Some(0.0),
                scale: Some(0.0000275),
                offset: Some(-0.2),
            },
            raw: raw.into(),
            slice_label: String::new(),
        }
    }

    #[test]
    fn the_range_skips_nodata_and_follows_the_mode() {
        let s = slice(vec![0.0, 11_636.0, 20_000.0, 0.0]);
        let (lo, hi) = s.range(true).expect("range");
        assert!((lo - 0.11999).abs() < 1e-9 && (hi - 0.35).abs() < 1e-9);
        assert_eq!(s.range(false), Some((11_636.0, 20_000.0)));
        assert_eq!(s.nodata_cells(), 2);
        assert!(slice(vec![0.0; 4]).range(true).is_none());
    }

    #[test]
    fn a_cell_reads_by_row_and_column() {
        let s = slice(vec![1.0, 2.0, 3.0, 4.0]);
        assert_eq!(s.raw_at(1, 0), Some(3.0));
        assert_eq!(s.raw_at(2, 0), None);
        assert!(s.physical_at(0, 1).is_some());
    }

    #[test]
    fn a_page_is_capped_when_the_box_holds_more() {
        let page = LayerPage {
            source: SourceRef::new("places", "geometry"),
            features: Vec::new(),
            matching: 3,
        };
        assert!(page.capped());
        assert_eq!(page.source.key(), "places.geometry");
    }
}
