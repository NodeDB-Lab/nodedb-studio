//! The Spatial domain: geometry sources, features, viewport reads, and
//! georeferenced rasters.
//!
//! A layer is a way to draw a source, not a storage engine. A source is a
//! geometry field of a collection. An array with a GEOREF is a raster, not a
//! collection of point features, so the two are listed apart.

pub mod analyze;
mod base;
pub mod geo_ops;
pub mod geo_proj;
pub mod geo_raster;
pub mod sql;
pub mod text;
pub use base::*;
