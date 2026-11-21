//! Vector Space data: the vector indexes a connection holds, the records
//! they cover, and the nearest-neighbor search over them.
//!
//! A vector index is an attribute of one field of one collection. It is not
//! a storage engine, so every type here names the collection and the field
//! it belongs to. Distances come from the index's metric and rank ascending:
//! a smaller distance is closer. A distance is never a probability.

mod base;
pub mod index;
pub mod pca;
pub mod projection;
pub mod sql;
pub use base::*;
