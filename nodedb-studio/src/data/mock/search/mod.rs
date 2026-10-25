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
mod stemmer;

use crate::models::search::{SearchCollection, SearchSetup, SearchText, SearchVector};
use crate::models::vector::Metric;

pub use analysis::analysis as search_analyze;
pub use indexes::{SearchStore, overview as search_indexes, plan as search_rebuild_plan};
pub use run::search as search_run;

/// What the connection offers Search. No embedding provider is configured,
/// so a semantic search starts from explicit values or a stored record.
pub fn search_setup() -> SearchSetup {
    SearchSetup {
        collections: vec![SearchCollection {
            name: indexes::COLLECTION.into(),
            id_field: "_id".into(),
            text: Some(SearchText {
                index: indexes::TEXT_INDEX.into(),
                fields: vec!["title".into(), "body".into()],
                analyzer: "English".into(),
                binding_revision: analysis::BINDING_REVISION,
            }),
            vector: Some(SearchVector {
                field: "embedding".into(),
                index: indexes::VECTOR_INDEX.into(),
                dimensions: 768,
                metric: Metric::Cosine,
            }),
            filter_fields: vec!["language".into(), "model".into()],
        }],
        provider: None,
    }
}

/// A query vector near `art104`, as one a person would attach: the stored
/// embedding of that record, nudged by a fixed pseudo-random offset so its
/// distance to the record is small and not zero.
pub fn attached_query_vector() -> Vec<f32> {
    let base = crate::data::mock::vector_record("articles", "embedding", "art104")
        .map(|r| r.vector)
        .unwrap_or_default();
    let mut state: u32 = 0x9E37_79B9;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state as f32 / u32::MAX as f32 - 0.5
    };
    let nudged: Vec<f32> = base.iter().map(|x| x + 0.0535 * next()).collect();
    let norm = nudged.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm == 0.0 {
        return nudged;
    }
    nudged.iter().map(|x| x / norm).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::search::{Score, SearchMode, SearchRequest, VectorInput};

    #[test]
    fn the_setup_names_the_two_indexes_of_articles() {
        let s = search_setup();
        let c = s.collection("articles").expect("articles");
        assert_eq!(
            c.text.as_ref().map(|t| t.index.as_str()),
            Some("idx_articles_text")
        );
        assert_eq!(
            c.vector.as_ref().map(|v| v.index.as_str()),
            Some("idx_articles_embedding")
        );
        assert!(SearchMode::ALL.iter().all(|m| c.supports(*m)));
    }

    #[test]
    fn the_attached_vector_is_close_to_art104_and_not_equal() {
        let v = attached_query_vector();
        assert_eq!(v.len(), 768);
        let req = SearchRequest {
            collection: "articles".into(),
            mode: SearchMode::Semantic,
            query: String::new(),
            text_field: None,
            vector_field: Some("embedding".into()),
            vector: VectorInput::Values(v),
            filters: Vec::new(),
            limit: 5,
            fusion: Default::default(),
        };
        let r = search_run(&req, 1).expect("searches");
        assert_eq!(r.hits[0].id, "art104");
        let Score::Distance(d) = r.hits[0].score else {
            panic!("a semantic score is a distance");
        };
        assert!((0.07..0.10).contains(&d), "distance {d}");
    }
}
