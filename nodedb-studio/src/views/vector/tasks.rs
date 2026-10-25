//! Vector Space writes that outlive a click. Each runs under `spawn_forever`
//! and touches root state only.

pub use crate::views::query::tasks::open_in_query;
