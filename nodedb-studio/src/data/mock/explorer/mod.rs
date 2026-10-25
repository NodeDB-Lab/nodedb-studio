//! Explorer fixtures: the catalog, schemas, record datasets, edges, and the
//! array. Each dataset yields any row by index, so a collection of 12,000
//! rows costs nothing until a page reads it.

mod access;
mod analytics;
mod array;
mod build;
mod catalog;
mod docs;
mod edges;
mod places;
mod query;
mod schemas;
mod sessions;
mod shipments;

pub use access::collection_access;
pub use array::{array_schema, array_slice};
pub use catalog::collections;
pub use edges::{GRAPH, edge_page};
pub use query::{Overlay, observed_at, record_page, stored_record, transfer_preview};
pub use schemas::collection_schema;
