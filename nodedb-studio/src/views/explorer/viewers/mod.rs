//! One purpose-built viewer per NodeDB storage mode. The Explorer swaps
//! between these based on the selected collection's mode.

pub mod document;
pub mod fts;
pub mod graph;
pub mod kv;
pub mod spatial;
pub mod strict;
pub mod timeseries;
pub mod vector;
