//! Search reads and writes. Ranking, analysis, and fusion are computed over
//! the articles. Rebuild jobs a person creates live in memory.

use async_trait::async_trait;

use crate::data::mock;
use crate::models::search::{SearchRequest, SearchResult, SearchSetup};
use crate::models::search_analyze::{Analysis, AnalyzeRequest};
use crate::models::search_index::{BuildRun, IndexOverview, RebuildPlan, RunHistory};
use crate::services::error::StudioError;
use crate::services::mock::MockConnectionService;
use crate::services::mock_behavior::{MockBehavior, apply_one, apply_one_result};
use crate::services::search_data::SearchData;

#[async_trait(?Send)]
impl SearchData for MockConnectionService {
    async fn search_setup(&self) -> Result<SearchSetup, StudioError> {
        let mut setup = mock::search_setup();
        if self.behavior == MockBehavior::Empty {
            setup.collections.clear();
        }
        apply_one_result(self.behavior, Ok(setup)).await
    }

    async fn search_run(&self, request: &SearchRequest) -> Result<SearchResult, StudioError> {
        let execution = self.search.borrow_mut().next_execution();
        let mut read = mock::search_run(request, execution);
        if self.behavior == MockBehavior::Empty
            && let Ok(r) = &mut read
        {
            r.hits.clear();
        }
        apply_one_result(self.behavior, read).await
    }

    async fn search_analyze(&self, request: &AnalyzeRequest) -> Result<Analysis, StudioError> {
        apply_one_result(self.behavior, mock::search_analyze(request)).await
    }

    async fn search_indexes(&self, collection: &str) -> Result<IndexOverview, StudioError> {
        let mut read = mock::search_indexes(collection);
        if self.behavior == MockBehavior::Empty
            && let Ok(o) = &mut read
        {
            o.indexes.clear();
        }
        apply_one_result(self.behavior, read).await
    }

    async fn search_build_history(&self, index: &str) -> Result<RunHistory, StudioError> {
        let mut read = self.search.borrow().history(index);
        if self.behavior == MockBehavior::Empty
            && let Ok(h) = &mut read
        {
            h.runs.clear();
        }
        apply_one_result(self.behavior, read).await
    }

    async fn search_build_run(&self, id: &str) -> Result<BuildRun, StudioError> {
        let read = self.search.borrow().run(id);
        apply_one_result(self.behavior, read).await
    }

    async fn search_rebuild_plan(&self, index: &str) -> Result<RebuildPlan, StudioError> {
        apply_one_result(self.behavior, mock::search_rebuild_plan(index)).await
    }

    async fn search_start_rebuild(
        &self,
        index: &str,
        target_revision: u32,
    ) -> Result<BuildRun, StudioError> {
        // A write either applies or errors: an erroring connection changes nothing.
        apply_one(self.behavior, || ()).await?;
        self.search
            .borrow_mut()
            .start_rebuild(index, target_revision)
    }
}
