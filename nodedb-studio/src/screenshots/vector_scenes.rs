//! The Vector Space states the design shows. Each seed names every tab's
//! draft, so one scene's result never leaks into the next image.
//!
//! A seed holds the request a tab starts with, not a result: the page runs
//! it through the backend, so the screen shows computed values.

use crate::models::records::Filter;
use crate::models::vector::{NeighborRequest, QuerySource, SearchForm};
use crate::models::vector_projection::{Method, ProjectionForm, ProjectionRequest};
use crate::state::vector::{ProjectionUi, SearchUi, Selection, VectorSeed};

pub const TARGET: &str = "articles.embedding";
/// The record the Search scene queries from.
const QUERY_RECORD: &str = "art104";
/// The record the Projection scene inspects.
const SELECTED: &str = "art103";

/// Search: the neighbors of `art104`, two of them, its own record excluded.
pub fn search() -> VectorSeed {
    let request = NeighborRequest {
        collection: "articles".into(),
        field: "embedding".into(),
        source: QuerySource::Record(QUERY_RECORD.into()),
        top_k: 2,
        exclude_source: true,
        filters: Vec::<Filter>::new(),
        ef_search: None,
    };
    VectorSeed {
        search: Some(SearchUi {
            form: SearchForm {
                record_id: QUERY_RECORD.into(),
                top_k: "2".into(),
                ..SearchForm::default()
            },
            pending: Some((1, request)),
            seq: 1,
            ..SearchUi::default()
        }),
        ..VectorSeed::default()
    }
}

/// Projection: a seeded sample of 5,000, with `art103` in it and selected.
pub fn projection() -> VectorSeed {
    let request = ProjectionRequest {
        collection: "articles".into(),
        field: "embedding".into(),
        method: Method::Pca,
        sample: 5_000,
        seed: 42,
        color_by: None,
        include: vec![SELECTED.into()],
    };
    VectorSeed {
        projection: Some(ProjectionUi {
            form: ProjectionForm::default(),
            pending: Some((1, request)),
            seq: 1,
            ..ProjectionUi::default()
        }),
        selected: Some(Selection {
            target: TARGET.into(),
            id: SELECTED.into(),
        }),
        ..VectorSeed::default()
    }
}

/// Index: the snapshot loads by itself.
pub fn index() -> VectorSeed {
    VectorSeed::default()
}
