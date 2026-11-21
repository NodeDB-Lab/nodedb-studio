//! Search: collection-backed retrieval over the records a connection holds.
//!
//! A full-text index and a vector index are attributes of fields of one
//! collection, never a collection of their own. A search names the
//! collection, the retrieval mode, the fields, the query inputs, and the
//! filters, and answers with ranked records. A ranked result is a view of
//! the source records: its rank and score belong to one execution and never
//! to the record.
//!
//! Keyword ranks by BM25 (larger is better). Semantic ranks by the vector
//! index's distance (smaller is closer). Hybrid ranks by a fusion of the two
//! ranked lists, so its score is rank-based and not comparable to either raw
//! score.

pub mod analyze;
mod base;
pub mod compare;
pub mod form;
pub mod index;
pub mod sql;
pub use base::*;
