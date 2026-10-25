//! The Explorer's article dataset, as Vector Space and Search read it.

use super::docs;

/// How many articles the Explorer holds. Vector Space reads the same records.
pub const ARTICLE_COUNT: usize = docs::ARTICLES_SET.total;

/// Article `i` in the Explorer's natural order, newest first.
pub fn article_row(i: usize) -> crate::models::records::Record {
    (docs::ARTICLES_SET.row)(i)
}
