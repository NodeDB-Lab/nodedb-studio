//! Vector Space fixtures: the `articles.embedding` index, its records, a
//! brute-force search, a computed projection, and an index snapshot.

mod corpus;
mod index;
mod project;
mod records;
mod search;
mod setup;

pub use index::index_status as vector_index_status;
pub use project::project as vector_project;
pub use records::{ArticleText, RESIDENT_ARTICLES, article_number, article_text, vector_record};
pub use search::search as vector_search;
pub use setup::vector_setup;
