//! Materialized views against a real server. Not wired yet.

use async_trait::async_trait;

use crate::models::matviews::{MaterializedView, RefreshRun, ViewRows};
use crate::services::error::StudioError;
use crate::services::matview_data::MatViewData;
use crate::services::nodedb::NodeDbConnectionService;

#[async_trait(?Send)]
impl MatViewData for NodeDbConnectionService {
    async fn materialized_views(&self) -> Result<Vec<MaterializedView>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn view_rows(
        &self,
        _view: &str,
        _page: usize,
        _page_size: usize,
    ) -> Result<ViewRows, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn refresh_history(&self, _view: &str) -> Result<Vec<RefreshRun>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn refresh_view(&self, _view: &str) -> Result<RefreshRun, StudioError> {
        Err(StudioError::NotConnected)
    }
}
