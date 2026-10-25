//! Array collections: dimensions, attributes, and slices.
//!
//! An array is not a table. A slice fixes one coordinate on every
//! dimension but two, and returns the cells between the other two.

/// One dimension of an array.
#[derive(Debug, Clone, PartialEq)]
pub struct Dimension {
    pub name: String,
    /// First coordinate.
    pub start: f64,
    /// Last coordinate, inclusive.
    pub end: f64,
    /// Distance between neighbouring coordinates.
    pub step: f64,
    pub points: u32,
    pub chunk: u32,
    /// What a coordinate measures, `degrees north` or `hour (UTC)`.
    pub unit: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    pub name: String,
    pub dtype: String,
    pub unit: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArraySchema {
    pub array: String,
    /// `time`, `lat`, `lon`.
    pub dimensions: Vec<Dimension>,
    pub attributes: Vec<Attribute>,
    /// The time axis starts here, seconds since the epoch.
    pub time_origin: i64,
    /// The time axis step, seconds.
    pub time_step: i64,
}

impl ArraySchema {
    pub fn dimension(&self, name: &str) -> Option<&Dimension> {
        self.dimensions.iter().find(|d| d.name == name)
    }

    pub fn shape(&self) -> Vec<u32> {
        self.dimensions.iter().map(|d| d.points).collect()
    }

    pub fn chunk_shape(&self) -> Vec<u32> {
        self.dimensions.iter().map(|d| d.chunk).collect()
    }
}

/// A slice of one attribute at one time.
#[derive(Debug, Clone, PartialEq)]
pub struct SliceRequest {
    /// Index on the time axis.
    pub time_index: u32,
    pub lat: (f64, f64),
    pub lon: (f64, f64),
    pub attribute: String,
}

/// The cells of a slice. `values[row][col]` is the cell at `lats[row]`,
/// `lons[col]`, with latitude descending so north is up.
#[derive(Debug, Clone, PartialEq)]
pub struct SliceResult {
    pub attribute: String,
    pub unit: String,
    pub lats: Vec<f64>,
    pub lons: Vec<f64>,
    pub values: Vec<Vec<f64>>,
    pub min: f64,
    pub max: f64,
}

impl SliceResult {
    pub fn cell(&self, row: usize, col: usize) -> Option<f64> {
        self.values.get(row)?.get(col).copied()
    }
}
