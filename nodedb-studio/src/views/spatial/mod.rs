//! Spatial: Map, Analyze, and Raster over the geometry sources and
//! georeferenced arrays a connection holds.

mod actions;
mod analyze;
mod analyze_form;
mod analyze_query;
mod analyze_results;
mod cell;
mod feature;
mod layers;
mod map;
mod map_canvas;
mod map_draw;
mod map_input;
mod map_overlay;
mod map_svg;
mod map_toolbar;
mod page;
mod raster;
mod raster_canvas;
mod raster_toolbar;
mod target_bar;

pub use page::SpatialPage;
