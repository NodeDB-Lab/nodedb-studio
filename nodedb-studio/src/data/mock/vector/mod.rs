//! Vector Space fixtures: the `articles.embedding` index, its records, a
//! brute-force search, a computed projection, and an index snapshot.

mod corpus;
mod index;
mod project;
mod search;

use crate::models::vector::{Metric, VectorRecord, VectorSetup, VectorTarget};
use crate::services::error::StudioError;
use corpus::{
    COLLECTION, DIMENSIONS, FIELD, INDEX, MODEL, POPULATION, REVISION, meta, n_of, vector_of,
};

pub use index::index_status as vector_index_status;
pub use project::project as vector_project;
pub use search::search as vector_search;

/// The vector indexes the connection holds. No embedding provider is
/// configured, so a search starts from a record or a pasted vector.
pub fn vector_setup() -> VectorSetup {
    VectorSetup {
        targets: vec![VectorTarget {
            collection: COLLECTION.into(),
            field: FIELD.into(),
            index: INDEX.into(),
            id_field: "_id".into(),
            dimensions: DIMENSIONS as u32,
            metric: Metric::from_name("cosine").unwrap_or(Metric::Cosine),
            model: Some(MODEL.into()),
            population: POPULATION,
            revision: REVISION,
            filter_fields: vec!["language".into(), "model".into()],
            color_fields: vec!["language".into(), "model".into()],
        }],
        provider: None,
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_setup_names_the_explorers_index() {
        let s = vector_setup();
        let t = s.target("articles", "embedding").expect("target");
        assert_eq!(
            (t.index.as_str(), t.dimensions),
            ("idx_articles_embedding", 768)
        );
        assert_eq!(t.model.as_deref(), Some("support-embed-v2"));
        assert!(s.provider.is_none());
    }

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
