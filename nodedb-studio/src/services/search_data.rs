//! Search reads and writes at the backend seam: the collections a search can
//! use, ranked retrieval, analyzer tests, index definitions, and builds.
//!
//! A search is a read. It never changes a collection, an analyzer, or an
//! index. Rebuilding an index is the one write: it creates a tracked job from
//! a plan the caller reviewed, and the server refuses a plan whose target
//! revision is no longer the index's.

use async_trait::async_trait;

use crate::models::search::{SearchRequest, SearchResult, SearchSetup};
use crate::models::search_analyze::{Analysis, AnalyzeRequest};
use crate::models::search_index::{BuildRun, IndexOverview, RebuildPlan, RunHistory};
use crate::services::error::StudioError;

#[async_trait(?Send)]
pub trait SearchData {
    /// The collections that carry a text or vector index, with the fields,
    /// analyzer, and distance metric each search mode needs.
    async fn search_setup(&self) -> Result<SearchSetup, StudioError>;

    /// Rank records. Filters apply to every leg before the cut. A semantic
    /// query vector the index cannot use is `StudioError::Rejected`.
    async fn search_run(&self, request: &SearchRequest) -> Result<SearchResult, StudioError>;

    /// Run text through the analyzer bound to the index and report the tokens,
    /// their source spans, and each stage. Nothing is saved.
    async fn search_analyze(&self, request: &AnalyzeRequest) -> Result<Analysis, StudioError>;

    /// The text and vector indexes of a collection, with what the server
    /// observed about each.
    async fn search_indexes(&self, collection: &str) -> Result<IndexOverview, StudioError>;

    /// The builds of one index, newest first.
    async fn search_build_history(&self, index: &str) -> Result<RunHistory, StudioError>;

    /// One build by its stable id. `UnknownObject` when it does not exist.
    async fn search_build_run(&self, id: &str) -> Result<BuildRun, StudioError>;

    /// What a rebuild would do. Nothing is created.
    async fn search_rebuild_plan(&self, index: &str) -> Result<RebuildPlan, StudioError>;

    /// Create a tracked rebuild job. `StaleRevision` when `target_revision` is
    /// not the one the index targets now, and nothing is created.
    async fn search_start_rebuild(
        &self,
        index: &str,
        target_revision: u32,
    ) -> Result<BuildRun, StudioError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::search::{Fusion, SearchMode, VectorInput};
    use crate::models::search_analyze::InputRole;
    use crate::models::search_index::RunState;
    use crate::services::async_state::AsyncState;
    use crate::services::mock::MockConnectionService;

    fn request(mode: SearchMode) -> SearchRequest {
        SearchRequest {
            collection: "articles".into(),
            mode,
            query: "distributed database".into(),
            text_field: Some("body".into()),
            vector_field: Some("embedding".into()),
            vector: VectorInput::Record("art104".into()),
            filters: Vec::new(),
            limit: 8,
            fusion: Fusion::default(),
        }
    }

    fn analyze() -> AnalyzeRequest {
        AnalyzeRequest {
            collection: "articles".into(),
            index: "idx_articles_text".into(),
            field: "body".into(),
            text: "The Distributed Databases".into(),
            role: InputRole::Query,
        }
    }

    #[tokio::test]
    async fn setup_collection_names_are_unique() {
        let svc = MockConnectionService::ready();
        let setup = svc.search_setup().await.expect("setup");
        let mut names: Vec<&str> = setup.collections.iter().map(|c| c.name.as_str()).collect();
        let n = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!((names.len(), n), (1, 1));
    }

    #[tokio::test]
    async fn setup_empty_has_no_collections() {
        let svc = MockConnectionService::empty();
        let s = AsyncState::from_value(Some(svc.search_setup().await));
        assert!(s.is_empty());
    }

    #[tokio::test]
    async fn setup_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.search_setup().await));
        assert!(s.error_message().is_some());
    }

    #[tokio::test]
    async fn run_ready_ranks_every_mode_with_unique_record_ids() {
        let svc = MockConnectionService::ready();
        for mode in SearchMode::ALL {
            let r = svc.search_run(&request(mode)).await.expect("search");
            let mut ids: Vec<&str> = r.hits.iter().map(|h| h.id.as_str()).collect();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(ids.len(), 8, "{mode:?}: every hit needs a stable key");
            assert_eq!(r.request, request(mode));
            assert!(
                r.hits
                    .iter()
                    .enumerate()
                    .all(|(i, h)| h.rank == i as u32 + 1)
            );
        }
    }

    #[tokio::test]
    async fn run_executions_count_up_and_bind_a_result() {
        let svc = MockConnectionService::ready();
        let a = svc
            .search_run(&request(SearchMode::Keyword))
            .await
            .expect("a");
        let b = svc
            .search_run(&request(SearchMode::Keyword))
            .await
            .expect("b");
        assert_eq!(b.execution, a.execution + 1);
        assert_eq!(a.hits, b.hits, "the same query ranks the same way");
    }

    #[tokio::test]
    async fn run_empty_returns_no_hits() {
        let svc = MockConnectionService::empty();
        let r = svc
            .search_run(&request(SearchMode::Hybrid))
            .await
            .expect("search");
        assert!(r.hits.is_empty());
    }

    #[tokio::test]
    async fn run_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.search_run(&request(SearchMode::Keyword)).await));
        assert!(s.error_message().is_some());
    }

    #[tokio::test]
    async fn analyze_ready_reports_tokens_with_spans() {
        let svc = MockConnectionService::ready();
        let a = svc.search_analyze(&analyze()).await.expect("analysis");
        assert_eq!(a.tokens.len(), 2);
        assert!(a.spans_match_the_text());
        assert_eq!(a.profile.binding_revision, 7);
    }

    #[tokio::test]
    async fn analyze_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.search_analyze(&analyze()).await));
        assert!(s.error_message().is_some());
    }

    #[tokio::test]
    async fn indexes_ready_lists_both_with_unique_names() {
        let svc = MockConnectionService::ready();
        let o = svc.search_indexes("articles").await.expect("indexes");
        let mut names: Vec<&str> = o.indexes.iter().map(|i| i.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 2);
    }

    #[tokio::test]
    async fn indexes_empty_is_empty_and_erroring_is_err() {
        let empty = MockConnectionService::empty();
        let s = AsyncState::from_value(Some(empty.search_indexes("articles").await));
        assert!(s.is_empty());
        let bad = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(bad.search_indexes("articles").await));
        assert!(s.error_message().is_some());
    }

    #[tokio::test]
    async fn history_has_unique_run_ids_and_erroring_is_err() {
        let svc = MockConnectionService::ready();
        let h = svc
            .search_build_history("idx_articles_text")
            .await
            .expect("history");
        let mut ids: Vec<&str> = h.runs.iter().map(|r| r.id.as_str()).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n);
        let bad = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(bad.search_build_history("idx_articles_text").await));
        assert!(s.error_message().is_some());
    }

    #[tokio::test]
    async fn a_rebuild_creates_one_queued_job_every_read_sees() {
        let svc = MockConnectionService::ready();
        let plan = svc
            .search_rebuild_plan("idx_articles_text")
            .await
            .expect("plan");
        let job = svc
            .search_start_rebuild(&plan.index, plan.target_revision)
            .await
            .expect("job");
        assert_eq!(job.state, RunState::Queued);
        assert_eq!(svc.search_build_run(&job.id).await.expect("run"), job);
        let h = svc
            .search_build_history("idx_articles_text")
            .await
            .expect("history");
        assert_eq!(h.runs[0].id, job.id);
        let again = svc
            .search_start_rebuild(&plan.index, plan.target_revision)
            .await;
        assert!(
            again.is_err(),
            "a second build while one is queued is refused"
        );
    }

    #[tokio::test]
    async fn a_stale_plan_creates_no_job() {
        let svc = MockConnectionService::ready();
        let err = svc
            .search_start_rebuild("idx_articles_text", 6)
            .await
            .expect_err("stale");
        assert!(matches!(err, StudioError::StaleRevision { .. }));
    }

    #[tokio::test]
    async fn writes_fail_on_an_erroring_connection() {
        let svc = MockConnectionService::erroring();
        let s =
            AsyncState::from_value(Some(svc.search_start_rebuild("idx_articles_text", 7).await));
        assert!(s.error_message().is_some());
    }
}
