//! Search reads and writes. The real client does not expose them yet.

use async_trait::async_trait;

use crate::models::search::{SearchRequest, SearchResult, SearchSetup};
use crate::models::search_analyze::{Analysis, AnalyzeRequest};
use crate::models::search_index::{BuildRun, IndexOverview, RebuildPlan, RunHistory};
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;
use crate::services::search_data::SearchData;

#[async_trait(?Send)]
impl SearchData for NodeDbConnectionService {
    async fn search_setup(&self) -> Result<SearchSetup, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn search_run(&self, _request: &SearchRequest) -> Result<SearchResult, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn search_analyze(&self, _request: &AnalyzeRequest) -> Result<Analysis, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn search_indexes(&self, _collection: &str) -> Result<IndexOverview, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn search_build_history(&self, _index: &str) -> Result<RunHistory, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn search_build_run(&self, _id: &str) -> Result<BuildRun, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn search_rebuild_plan(&self, _index: &str) -> Result<RebuildPlan, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn search_start_rebuild(
        &self,
        _index: &str,
        _target_revision: u32,
    ) -> Result<BuildRun, StudioError> {
        Err(StudioError::NotConnected)
    }
}
