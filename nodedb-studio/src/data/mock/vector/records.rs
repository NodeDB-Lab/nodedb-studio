//! Stored article records: the embedding a record carries and the text the
//! keyword index reads.

use super::corpus::{self, id_of, meta, n_of, vector_of};
use super::search;
use crate::models::vector::VectorRecord;
use crate::services::error::StudioError;

/// A stored article and its embedding.
pub fn vector_record(collection: &str, field: &str, id: &str) -> Result<VectorRecord, StudioError> {
    search::check_target(collection, field)?;
    let n = n_of(id).ok_or_else(|| StudioError::RecordMissing {
        collection: collection.to_string(),
        id: id.to_string(),
    })?;
    let m = meta(n);
    Ok(VectorRecord {
        collection: collection.to_string(),
        id: id.to_string(),
        title: m.title,
        body: m.body,
        language: m.language.map(str::to_string),
        model: Some(m.model.to_string()),
        embedded_at: Some(m.embedded_at),
        vector: vector_of(n),
    })
}

/// How many articles a mock search scans: the ones with a resident vector.
pub const RESIDENT_ARTICLES: u64 = corpus::RESIDENT;

/// The text of one article, for the keyword index built over the same
/// records the vector index holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArticleText {
    pub id: String,
    pub title: String,
    pub body: String,
    pub language: Option<&'static str>,
    pub model: &'static str,
}

/// Article `n`, in `1..=RESIDENT_ARTICLES`.
pub fn article_text(n: u64) -> ArticleText {
    let m = meta(n);
    ArticleText {
        id: id_of(n),
        title: m.title,
        body: m.body,
        language: m.language,
        model: m.model,
    }
}

/// The number of the article with this id, when the index holds it.
pub fn article_number(id: &str) -> Option<u64> {
    n_of(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_record_carries_its_text_and_vector() {
        let r = vector_record("articles", "embedding", "art104").expect("record");
        assert_eq!(r.title, "Distributed databases");
        assert_eq!(r.vector.len(), 768);
        assert!(r.language.is_some());
    }

    #[test]
    fn a_missing_record_is_named() {
        let err = vector_record("articles", "embedding", "art0").expect_err("missing");
        assert!(matches!(err, StudioError::RecordMissing { .. }));
    }
}
