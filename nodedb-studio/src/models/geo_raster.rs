//! Georeferenced array rasters in plain Rust: the affine GEOREF transform,
//! raw and physical values with NoData, color palettes, and the frame that
//! relates cells to what the canvas draws.
//!
//! A cell has an integer identity `(row, col)` and, through the GEOREF, a
//! world position. The two never mix: picking inverts the affine, and the
//! centre of a cell sits half a pixel in from its corner.

use crate::models::geo_proj::{mercator, unproject};

/// The GEOREF affine `[x0, a, b, y0, c, d]`:
/// `x = x0 + a*col + b*row` and `y = y0 + c*col + d*row`, where `(col, row)`
/// is a pixel corner. A north-up grid has `b = c = 0` and `d < 0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine {
    pub x0: f64,
    pub a: f64,
    pub b: f64,
    pub y0: f64,
    pub c: f64,
    pub d: f64,
}

impl Affine {
    pub const fn new(coeffs: [f64; 6]) -> Self {
        Self {
            x0: coeffs[0],
            a: coeffs[1],
            b: coeffs[2],
            y0: coeffs[3],
            c: coeffs[4],
            d: coeffs[5],
        }
    }

    pub const fn coeffs(self) -> [f64; 6] {
        [self.x0, self.a, self.b, self.y0, self.c, self.d]
    }

    fn det(self) -> f64 {
        self.a * self.d - self.b * self.c
    }

    /// Whether the transform has an inverse: every coefficient finite and
    /// the pixel vectors not parallel.
    pub fn is_invertible(self) -> bool {
        self.coeffs().iter().all(|v| v.is_finite()) && self.det().abs() > 1e-18
    }

    /// The world position of a pixel corner. `col` and `row` may be
    /// fractional.
    pub fn world(self, col: f64, row: f64) -> (f64, f64) {
        (
            self.x0 + self.a * col + self.b * row,
            self.y0 + self.c * col + self.d * row,
        )
    }

    /// The world position of a cell's centre.
    pub fn centre(self, row: u32, col: u32) -> (f64, f64) {
        self.world(f64::from(col) + 0.5, f64::from(row) + 0.5)
    }

    /// The fractional `(col, row)` at a world position. `None` when the
    /// transform has no inverse.
    pub fn pixel(self, x: f64, y: f64) -> Option<(f64, f64)> {
        if !self.is_invertible() {
            return None;
        }
        let det = self.det();
        let (dx, dy) = (x - self.x0, y - self.y0);
        Some((
            (self.d * dx - self.b * dy) / det,
            (self.a * dy - self.c * dx) / det,
        ))
    }

    /// The cell that holds a world position, or `None` outside the grid.
    pub fn cell_at(self, x: f64, y: f64, rows: u32, cols: u32) -> Option<(u32, u32)> {
        let (col, row) = self.pixel(x, y)?;
        if !(0.0..f64::from(cols)).contains(&col) || !(0.0..f64::from(rows)).contains(&row) {
            return None;
        }
        Some((row.floor() as u32, col.floor() as u32))
    }

    /// The length of one pixel along a column step and along a row step.
    pub fn pixel_size(self) -> (f64, f64) {
        (self.a.hypot(self.c), self.b.hypot(self.d))
    }

    /// Whether the grid has no rotation or shear and rows run south.
    pub fn is_north_up(self) -> bool {
        self.b == 0.0 && self.c == 0.0 && self.a > 0.0 && self.d < 0.0
    }

    /// The four corners of a `rows` by `cols` grid, clockwise from the
    /// top left.
    pub fn corners(self, rows: u32, cols: u32) -> [(f64, f64); 4] {
        let (r, c) = (f64::from(rows), f64::from(cols));
        [
            self.world(0.0, 0.0),
            self.world(c, 0.0),
            self.world(c, r),
            self.world(0.0, r),
        ]
    }
}

/// What a band declares about its values.
#[derive(Debug, Clone, PartialEq)]
pub struct BandMeta {
    pub name: String,
    pub dtype: String,
    pub units: String,
    /// The raw value that marks a cell as holding no data.
    pub nodata: Option<f64>,
    pub scale: Option<f64>,
    pub offset: Option<f64>,
}

impl BandMeta {
    /// Whether a raw value is the NoData marker. A `float32` band compares at
    /// `f32` width, and `NaN` matches `NaN`.
    pub fn is_nodata(&self, raw: f64) -> bool {
        let Some(nd) = self.nodata else { return false };
        if nd.is_nan() {
            return raw.is_nan();
        }
        if self.dtype.eq_ignore_ascii_case("float32") {
            return (raw as f32) == (nd as f32);
        }
        raw == nd
    }

    /// `raw * scale + offset`. A missing scale reads as 1 and a missing
    /// offset as 0. NoData is tested first and reads as `None`: it is
    /// missing data, never a measured value.
    pub fn physical(&self, raw: f64) -> Option<f64> {
        if self.is_nodata(raw) {
            return None;
        }
        Some(raw * self.scale.unwrap_or(1.0) + self.offset.unwrap_or(0.0))
    }
}

/// A continuous color ramp for physical values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Palette {
    #[default]
    Viridis,
    Magma,
}

const VIRIDIS: [[u8; 3]; 9] = [
    [68, 1, 84],
    [71, 44, 122],
    [59, 81, 139],
    [44, 113, 142],
    [33, 144, 141],
    [39, 173, 129],
    [92, 200, 99],
    [170, 220, 50],
    [253, 231, 37],
];

const MAGMA: [[u8; 3]; 9] = [
    [0, 0, 4],
    [28, 16, 68],
    [79, 18, 123],
    [129, 37, 129],
    [181, 54, 122],
    [229, 80, 100],
    [251, 135, 97],
    [254, 194, 135],
    [252, 253, 191],
];

impl Palette {
    pub const ALL: [Palette; 2] = [Palette::Viridis, Palette::Magma];

    pub fn label(self) -> &'static str {
        match self {
            Palette::Viridis => "Viridis",
            Palette::Magma => "Magma",
        }
    }

    fn stops(self) -> &'static [[u8; 3]; 9] {
        match self {
            Palette::Viridis => &VIRIDIS,
            Palette::Magma => &MAGMA,
        }
    }

    /// The color at `t`, from 0 to 1. A value outside clamps to an end.
    pub fn color(self, t: f64) -> [u8; 3] {
        let stops = self.stops();
        let t = if t.is_finite() {
            t.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let at = t * (stops.len() - 1) as f64;
        let i = (at.floor() as usize).min(stops.len() - 2);
        let f = at - i as f64;
        let mix = |k: usize| {
            let (lo, hi) = (f64::from(stops[i][k]), f64::from(stops[i + 1][k]));
            (lo + (hi - lo) * f).round() as u8
        };
        [mix(0), mix(1), mix(2)]
    }

    /// The ramp as a CSS gradient over `n` stops, for a legend.
    pub fn css_gradient(self) -> String {
        let stops = self.stops();
        let parts: Vec<String> = stops
            .iter()
            .enumerate()
            .map(|(i, c)| {
                format!(
                    "rgb({},{},{}) {:.1}%",
                    c[0],
                    c[1],
                    c[2],
                    i as f64 / (stops.len() - 1) as f64 * 100.0
                )
            })
            .collect();
        format!("linear-gradient(90deg, {})", parts.join(", "))
    }
}

/// Raw stored values, or physical values after scale and offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ValueMode {
    Raw,
    #[default]
    Physical,
}

impl ValueMode {
    pub const ALL: [ValueMode; 2] = [ValueMode::Physical, ValueMode::Raw];

    pub fn label(self) -> &'static str {
        match self {
            ValueMode::Raw => "Raw",
            ValueMode::Physical => "Physical",
        }
    }
}

/// How the canvas shows the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RasterCrs {
    /// The source projection, with the GEOREF's own pixel shape.
    #[default]
    Native,
    /// Web Mercator, resampled from a geographic source.
    WebMercator,
}

impl RasterCrs {
    pub const ALL: [RasterCrs; 2] = [RasterCrs::Native, RasterCrs::WebMercator];

    pub fn label(self) -> &'static str {
        match self {
            RasterCrs::Native => "Native",
            RasterCrs::WebMercator => "EPSG:3857",
        }
    }
}

/// How cells relate to the picture the canvas draws.
///
/// The drawn image is an RGBA bitmap with its own affine into the display
/// plane. Native display is the grid as is, and Web Mercator display is the
/// grid resampled, so a pick and a marker both go through the display plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RasterFrame {
    pub rows: u32,
    pub cols: u32,
    pub source: Affine,
    pub srid: u32,
    pub crs: RasterCrs,
}

impl RasterFrame {
    /// Whether the grid can show in `crs`. A source that is not geographic
    /// needs a CRS transform Studio does not have.
    pub fn supports(&self, crs: RasterCrs) -> Result<(), String> {
        match crs {
            RasterCrs::Native => Ok(()),
            RasterCrs::WebMercator if self.srid == 4326 && self.source.is_north_up() => Ok(()),
            RasterCrs::WebMercator if self.srid == 4326 => Err(
                "This grid is rotated, and Studio resamples only north-up grids to EPSG:3857."
                    .to_string(),
            ),
            RasterCrs::WebMercator => Err(format!(
                "A transform from EPSG:{} to EPSG:3857 is not available here.",
                self.srid
            )),
        }
    }

    /// The bitmap's size in pixels.
    pub fn image_size(&self) -> (u32, u32) {
        match self.crs {
            RasterCrs::Native => (self.cols, self.rows),
            RasterCrs::WebMercator => {
                let (min_x, min_y, max_x, max_y) = self.merc_bounds();
                let aspect = (max_y - min_y) / (max_x - min_x).max(f64::MIN_POSITIVE);
                let h = (f64::from(self.cols) * aspect).round().max(1.0);
                (self.cols, h as u32)
            }
        }
    }

    /// The bitmap's affine into the display plane.
    pub fn image_affine(&self) -> Affine {
        match self.crs {
            RasterCrs::Native => self.source,
            RasterCrs::WebMercator => {
                let (min_x, min_y, max_x, max_y) = self.merc_bounds();
                let (w, h) = self.image_size();
                Affine::new([
                    min_x,
                    (max_x - min_x) / f64::from(w),
                    0.0,
                    max_y,
                    0.0,
                    -(max_y - min_y) / f64::from(h),
                ])
            }
        }
    }

    /// The footprint in Mercator metres of a north-up geographic grid.
    fn merc_bounds(&self) -> (f64, f64, f64, f64) {
        let [(x0, y0), (x1, _), (_, y1), _] = self.source.corners(self.rows, self.cols);
        let (west, north) = (x0.min(x1), y0.max(y1));
        let (east, south) = (x0.max(x1), y0.min(y1));
        let (min_x, min_y) = mercator(west, south);
        let (max_x, max_y) = mercator(east, north);
        (min_x, min_y, max_x, max_y)
    }

    /// The footprint in the display plane as `(min_x, min_y, max_x, max_y)`.
    pub fn bounds(&self) -> (f64, f64, f64, f64) {
        let (w, h) = self.image_size();
        let pts = self.image_affine().corners(h, w);
        let xs = pts.iter().map(|p| p.0);
        let ys = pts.iter().map(|p| p.1);
        (
            xs.clone().fold(f64::INFINITY, f64::min),
            ys.clone().fold(f64::INFINITY, f64::min),
            xs.fold(f64::NEG_INFINITY, f64::max),
            ys.fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// The cell under a display-plane position.
    pub fn cell_at(&self, x: f64, y: f64) -> Option<(u32, u32)> {
        match self.crs {
            RasterCrs::Native => self.source.cell_at(x, y, self.rows, self.cols),
            RasterCrs::WebMercator => {
                let (lon, lat) = unproject(x, y);
                self.source.cell_at(lon, lat, self.rows, self.cols)
            }
        }
    }

    /// The display-plane position of a cell's centre.
    pub fn cell_centre(&self, row: u32, col: u32) -> (f64, f64) {
        let (x, y) = self.source.centre(row, col);
        match self.crs {
            RasterCrs::Native => (x, y),
            RasterCrs::WebMercator => mercator(x, y),
        }
    }

    /// The display-plane extent of one cell, for the selection outline:
    /// its four corners.
    pub fn cell_corners(&self, row: u32, col: u32) -> [(f64, f64); 4] {
        let (r, c) = (f64::from(row), f64::from(col));
        let at = |dc: f64, dr: f64| {
            let (x, y) = self.source.world(c + dc, r + dr);
            match self.crs {
                RasterCrs::Native => (x, y),
                RasterCrs::WebMercator => mercator(x, y),
            }
        };
        [at(0.0, 0.0), at(1.0, 0.0), at(1.0, 1.0), at(0.0, 1.0)]
    }

    /// RGBA for the bitmap. `color(row, col)` is the cell's color, or `None`
    /// for NoData, which draws fully transparent. A bitmap pixel outside the
    /// grid is transparent too.
    pub fn render(&self, color: impl Fn(u32, u32) -> Option<[u8; 3]>) -> (u32, u32, Vec<u8>) {
        let (w, h) = self.image_size();
        let image = self.image_affine();
        let mut out = vec![0u8; (w * h * 4) as usize];
        for j in 0..h {
            for i in 0..w {
                let (x, y) = image.centre(j, i);
                let Some((row, col)) = self.cell_at(x, y) else {
                    continue;
                };
                if let Some([r, g, b]) = color(row, col) {
                    let at = ((j * w + i) * 4) as usize;
                    out[at..at + 4].copy_from_slice(&[r, g, b, 255]);
                }
            }
        }
        (w, h, out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The 512 by 512 scene the design describes.
    const SCENE: Affine = Affine::new([800_000.0, 30.0, 0.0, 360_000.0, 0.0, -30.0]);

    fn band() -> BandMeta {
        BandMeta {
            name: "red".into(),
            dtype: "uint16".into(),
            units: "reflectance".into(),
            nodata: Some(0.0),
            scale: Some(0.0000275),
            offset: Some(-0.2),
        }
    }

    #[test]
    fn a_cell_centre_sits_half_a_pixel_in() {
        let (e, n) = SCENE.centre(128, 256);
        assert_eq!((e, n), (807_695.0, 356_145.0));
        // The corner is half a pixel up and left.
        assert_eq!(SCENE.world(256.0, 128.0), (807_680.0, 356_160.0));
    }

    #[test]
    fn the_inverse_finds_the_cell_and_its_neighbours() {
        assert_eq!(
            SCENE.cell_at(807_695.0, 356_145.0, 512, 512),
            Some((128, 256))
        );
        // One metre inside the corner is still that cell, a metre outside is not.
        assert_eq!(
            SCENE.cell_at(807_680.5, 356_159.5, 512, 512),
            Some((128, 256))
        );
        assert_eq!(
            SCENE.cell_at(807_679.5, 356_159.5, 512, 512),
            Some((128, 255))
        );
        assert_eq!(SCENE.cell_at(799_999.0, 360_000.0, 512, 512), None);
        assert_eq!(SCENE.cell_at(800_000.0, 360_001.0, 512, 512), None);
    }

    #[test]
    fn every_centre_inverts_to_its_own_cell() {
        for (row, col) in [(0, 0), (0, 511), (511, 0), (511, 511), (255, 100)] {
            let (x, y) = SCENE.centre(row, col);
            assert_eq!(SCENE.cell_at(x, y, 512, 512), Some((row, col)));
        }
    }

    #[test]
    fn a_rotated_and_sheared_grid_inverts() {
        let g = Affine::new([500_000.0, 24.0, 7.0, 4_000_000.0, 9.0, -28.0]);
        assert!(g.is_invertible() && !g.is_north_up());
        let (x, y) = g.centre(40, 90);
        assert_eq!(g.cell_at(x, y, 100, 200), Some((40, 90)));
        let (col, row) = g.pixel(x, y).expect("inverse");
        assert!((col - 90.5).abs() < 1e-9 && (row - 40.5).abs() < 1e-9);
    }

    #[test]
    fn a_singular_affine_has_no_inverse() {
        let g = Affine::new([0.0, 1.0, 2.0, 0.0, 2.0, 4.0]);
        assert!(!g.is_invertible());
        assert!(g.pixel(1.0, 1.0).is_none());
        assert!(g.cell_at(1.0, 1.0, 10, 10).is_none());
    }

    #[test]
    fn pixel_size_and_orientation_come_from_the_coefficients() {
        assert_eq!(SCENE.pixel_size(), (30.0, 30.0));
        assert!(SCENE.is_north_up());
        assert!(!Affine::new([0.0, 1.0, 0.1, 0.0, 0.0, -1.0]).is_north_up());
    }

    #[test]
    fn physical_is_raw_times_scale_plus_offset() {
        let p = band().physical(11_636.0).expect("valid");
        assert!((p - 0.11999).abs() < 1e-9, "{p}");
        assert_eq!(format!("{p:.5}"), "0.11999");
    }

    #[test]
    fn nodata_is_tested_before_scaling() {
        // Raw 0 would scale to -0.2. It is missing data, not that value.
        assert_eq!(band().physical(0.0), None);
        assert!(band().is_nodata(0.0));
        // A valid low raw value is not NoData.
        assert!(band().physical(1.0).is_some());
    }

    #[test]
    fn missing_scale_and_offset_read_as_identity() {
        let b = BandMeta {
            scale: None,
            offset: None,
            nodata: None,
            ..band()
        };
        assert_eq!(b.physical(22.4), Some(22.4));
        assert_eq!(b.physical(0.0), Some(0.0), "no marker, so zero is data");
    }

    #[test]
    fn a_float_band_matches_nodata_at_f32_width_and_nan_matches_nan() {
        let b = BandMeta {
            dtype: "float32".into(),
            nodata: Some(-9999.0),
            scale: None,
            offset: None,
            ..band()
        };
        assert!(b.is_nodata(-9999.0));
        assert!(!b.is_nodata(-9998.5));
        let nan = BandMeta {
            nodata: Some(f64::NAN),
            ..b
        };
        assert!(nan.is_nodata(f64::NAN));
        assert!(!nan.is_nodata(0.0));
    }

    #[test]
    fn a_palette_runs_from_its_first_stop_to_its_last() {
        assert_eq!(Palette::Viridis.color(0.0), [68, 1, 84]);
        assert_eq!(Palette::Viridis.color(1.0), [253, 231, 37]);
        assert_eq!(Palette::Viridis.color(-3.0), [68, 1, 84]);
        assert_eq!(Palette::Viridis.color(9.0), [253, 231, 37]);
        assert_eq!(Palette::Viridis.color(0.5), [33, 144, 141]);
        assert_ne!(Palette::Magma.color(0.5), Palette::Viridis.color(0.5));
        assert!(
            Palette::Viridis
                .css_gradient()
                .starts_with("linear-gradient")
        );
    }

    fn geographic() -> RasterFrame {
        // climate_cube: 181 rows of latitude, 360 columns of longitude.
        RasterFrame {
            rows: 181,
            cols: 360,
            source: Affine::new([-92.05, 0.1, 0.0, 49.05, 0.0, -0.1]),
            srid: 4326,
            crs: RasterCrs::Native,
        }
    }

    #[test]
    fn the_geographic_grid_puts_the_reference_cell_at_its_coordinates() {
        let f = geographic();
        // 40.5 N, 73.5 W is row 85, column 185.
        let (lon, lat) = f.source.centre(85, 185);
        assert!((lon - -73.5).abs() < 1e-9 && (lat - 40.5).abs() < 1e-9);
        assert_eq!(f.cell_at(-73.5, 40.5), Some((85, 185)));
        // Row 0 is the northern edge, column 0 the western.
        let (lon, lat) = f.source.centre(0, 0);
        assert!((lon - -92.0).abs() < 1e-9 && (lat - 49.0).abs() < 1e-9);
    }

    #[test]
    fn display_and_cell_round_trip_in_both_crs() {
        for crs in RasterCrs::ALL {
            let f = RasterFrame {
                crs,
                ..geographic()
            };
            assert!(f.supports(crs).is_ok());
            for (row, col) in [(0, 0), (85, 185), (180, 359), (40, 300)] {
                let (x, y) = f.cell_centre(row, col);
                assert_eq!(f.cell_at(x, y), Some((row, col)), "{crs:?} {row},{col}");
            }
        }
    }

    #[test]
    fn mercator_display_stretches_rows_toward_the_pole() {
        let f = RasterFrame {
            crs: RasterCrs::WebMercator,
            ..geographic()
        };
        let (w, h) = f.image_size();
        assert_eq!(w, 360);
        // Latitudes 31 to 49 span more Mercator metres than degrees suggest.
        assert!(h > 181, "{h}");
        let (min_x, min_y, max_x, max_y) = f.bounds();
        assert!(((max_y - min_y) / (max_x - min_x) - f64::from(h) / f64::from(w)).abs() < 0.01);
    }

    #[test]
    fn a_projected_source_does_not_resample_to_mercator() {
        let f = RasterFrame {
            rows: 512,
            cols: 512,
            source: SCENE,
            srid: 32647,
            crs: RasterCrs::Native,
        };
        let why = f
            .supports(RasterCrs::WebMercator)
            .expect_err("no transform");
        assert!(why.contains("EPSG:32647"), "{why}");
        let rotated = RasterFrame {
            source: Affine::new([0.0, 0.1, 0.01, 0.0, 0.01, -0.1]),
            srid: 4326,
            ..f
        };
        assert!(rotated.supports(RasterCrs::WebMercator).is_err());
    }

    #[test]
    fn rendering_leaves_nodata_transparent() {
        let f = RasterFrame {
            rows: 2,
            cols: 3,
            source: Affine::new([0.0, 1.0, 0.0, 2.0, 0.0, -1.0]),
            srid: 4326,
            crs: RasterCrs::Native,
        };
        let (w, h, rgba) = f.render(|row, col| (!(row == 0 && col == 1)).then_some([10, 20, 30]));
        assert_eq!((w, h), (3, 2));
        assert_eq!(&rgba[4..8], &[0, 0, 0, 0], "NoData is transparent");
        assert_eq!(&rgba[0..4], &[10, 20, 30, 255]);
        assert_eq!(&rgba[20..24], &[10, 20, 30, 255]);
    }
}
