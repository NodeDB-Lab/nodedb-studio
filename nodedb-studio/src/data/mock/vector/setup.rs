//! The vector indexes the connection offers Vector Space.

use super::corpus::{COLLECTION, DIMENSIONS, FIELD, INDEX, MODEL, POPULATION, REVISION};
use crate::models::vector::{Metric, VectorSetup, VectorTarget};

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
}
