//! Materialized views: the list, rows, history, and a manual refresh.

use async_trait::async_trait;

use crate::models::matviews::{MaterializedView, RefreshRun, ViewRows};
use crate::services::error::StudioError;
use crate::services::matview_data::MatViewData;
use crate::services::mock::MockConnectionService;
use crate::services::mock_behavior::{
    MockBehavior, apply, apply_one_result, apply_result, apply_write,
};

#[async_trait(?Send)]
impl MatViewData for MockConnectionService {
    async fn materialized_views(&self) -> Result<Vec<MaterializedView>, StudioError> {
        apply(self.behavior, || self.views.borrow().views()).await
    }

    async fn view_rows(
        &self,
        view: &str,
        page: usize,
        page_size: usize,
    ) -> Result<ViewRows, StudioError> {
        let read = self.views.borrow().rows(view, page, page_size);
        match (self.behavior, read) {
            (MockBehavior::Empty, Ok(mut r)) => {
                r.rows.clear();
                r.total = 0;
                Ok(r)
            }
            (behavior, read) => apply_one_result(behavior, read).await,
        }
    }

    async fn refresh_history(&self, view: &str) -> Result<Vec<RefreshRun>, StudioError> {
        let read = self.views.borrow().history(view);
        apply_result(self.behavior, read).await
    }

    async fn refresh_view(&self, view: &str) -> Result<RefreshRun, StudioError> {
        apply_write(self.behavior, || self.views.borrow_mut().refresh(view)).await
    }
}
