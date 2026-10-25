//! Explorer fixtures: the catalog, schemas, record datasets, edges, and the
//! array. Each dataset yields any row by index, so a collection of 12,000
//! rows costs nothing until a page reads it.

mod access;
mod analytics;
mod array;
pub(super) mod build;
mod catalog;
mod docs;
mod edges;
mod places;
mod query;
mod schemas;
mod sessions;
mod shipments;

pub use access::collection_access;
pub use array::{array_schema, array_slice, sample as array_cell};
pub use catalog::collections;
pub use edges::{GRAPH, edge_page};
pub use query::{
    Overlay, all_records, observed_at, plain_text, record_page, stored_record, transfer_preview,
};
pub use schemas::collection_schema;

/// How many articles the Explorer holds. Vector Space reads the same records.
pub const ARTICLE_COUNT: usize = docs::ARTICLES_SET.total;

/// Article `i` in the Explorer's natural order, newest first.
pub fn article_row(i: usize) -> crate::models::records::Record {
    (docs::ARTICLES_SET.row)(i)
}
