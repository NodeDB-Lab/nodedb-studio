//! Catalog, record, and array access.
//!
//! The catalog and schema live here. Records, access, graph edges and arrays
//! are in `explorer::records`, `explorer::access`, `explorer::graph` and
//! `explorer::arrays`, called from the one `ExplorerData` impl below.

pub mod access;
pub mod arrays;
mod base;
pub mod graph;
pub mod records;
pub mod sql;
pub mod write;
pub(in crate::services::nodedb) use base::*;
