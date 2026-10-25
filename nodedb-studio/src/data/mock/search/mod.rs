//! Search fixtures: the `articles` collection's text and vector indexes, a
//! BM25 keyword search, Hybrid fusion over the Vector Space fixtures, an
//! analyzer, and the index build history.
//!
//! Everything a person sees is computed from the articles by plain Rust.
//! Nothing here is a stored result.

mod analysis;
mod analyzer;
mod bm25;
mod fusion;
mod indexes;
mod run;
mod setup;
mod stemmer;

pub use analysis::analysis as search_analyze;
pub use indexes::{SearchStore, overview as search_indexes, plan as search_rebuild_plan};
pub use run::search as search_run;
#[cfg(all(feature = "screenshots", target_os = "linux"))]
pub use setup::attached_query_vector;
pub use setup::search_setup;
