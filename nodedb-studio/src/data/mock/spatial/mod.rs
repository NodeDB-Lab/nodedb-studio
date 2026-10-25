//! Spatial fixtures. Geometry reads the Explorer's `places` records and
//! the raster reads its `climate_cube` array, so a feature or a cell has the
//! same coordinates and values on every screen.

mod analyze;
mod features;
mod raster;
mod setup;

pub use analyze::spatial_analyze;
pub use features::{spatial_feature, spatial_validity, spatial_viewport};
pub use raster::raster_slice;
pub use setup::spatial_setup;
