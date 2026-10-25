//! The Search states the design shows. Each seed names every tab's state, so
//! one scene's query, result, or selection never leaks into the next image.
//!
//! A seed holds what a person did, not a result: the pages read their data
//! through the backend, so the screens show computed rankings.

use crate::data::mock;
use crate::models::records::Filter;
use crate::models::search::{SearchMode, VectorInput};
use crate::models::search_analyze::{AnalyzeRequest, InputRole};
use crate::models::search_form::SearchForm;
use crate::state::search::{AnalyzeUi, IndexTab, IndexesUi, ResultsUi, SearchSeed};

const COLLECTION: &str = "articles";
const TEXT_INDEX: &str = "idx_articles_text";

/// Search: a Hybrid search for `distributed database` in English records,
/// with an attached query vector, run.
pub fn results() -> SearchSeed {
    let setup = mock::search_setup();
    let state = setup.collection(COLLECTION).and_then(|c| {
        let mut form = SearchForm {
            mode: SearchMode::Hybrid,
            query: "distributed database".into(),
            vector: VectorInput::Values(mock::attached_query_vector()),
            filters: vec![Filter {
                field: "language".into(),
                value: "en".into(),
            }],
            ..SearchForm::default()
        };
        form.settle(c);
        let request = form.check(c).ok()?;
        Some(ResultsUi {
            collection: Some(COLLECTION.into()),
            form,
            pending: Some((1, request)),
            seq: 1,
            ..ResultsUi::default()
        })
    });
    SearchSeed {
        results: state,
        ..SearchSeed::default()
    }
}

/// Analyze: `The Distributed Databases` through the bound English analyzer.
pub fn analyze() -> SearchSeed {
    let text = "The Distributed Databases";
    let request = AnalyzeRequest {
        collection: COLLECTION.into(),
        index: TEXT_INDEX.into(),
        field: "body".into(),
        text: text.into(),
        role: InputRole::Query,
    };
    SearchSeed {
        analyze: Some(AnalyzeUi {
            collection: Some(COLLECTION.into()),
            index: Some(TEXT_INDEX.into()),
            field: Some("body".into()),
            text: text.into(),
            role: InputRole::Query,
            pending: Some((1, request)),
            seq: 1,
            token: Some(0),
            ..AnalyzeUi::default()
        }),
        ..SearchSeed::default()
    }
}

/// Indexes: the text index selected, on its Definition tab.
pub fn indexes() -> SearchSeed {
    SearchSeed {
        indexes: Some(IndexesUi {
            collection: Some(COLLECTION.into()),
            selected: Some(TEXT_INDEX.into()),
            tab: IndexTab::Definition,
            ..IndexesUi::default()
        }),
        ..SearchSeed::default()
    }
}
