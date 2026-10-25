//! Materialized views at the backend seam: the saved views, their rows,
//! their refresh history, and a manual refresh.

use async_trait::async_trait;

use crate::models::matviews::{MaterializedView, RefreshRun, ViewRows};
use crate::services::error::StudioError;

#[async_trait(?Send)]
pub trait MatViewData {
    /// The materialized views of the selected database.
    async fn materialized_views(&self) -> Result<Vec<MaterializedView>, StudioError>;

    /// One page of a view's result, `page` counted from zero.
    async fn view_rows(
        &self,
        view: &str,
        page: usize,
        page_size: usize,
    ) -> Result<ViewRows, StudioError>;

    /// The view's refreshes, newest first.
    async fn refresh_history(&self, view: &str) -> Result<Vec<RefreshRun>, StudioError>;

    /// Start a manual refresh and return its run.
    async fn refresh_view(&self, view: &str) -> Result<RefreshRun, StudioError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::async_state::AsyncState;
    use crate::services::mock::MockConnectionService;

    #[tokio::test]
    async fn view_ids_are_unique() {
        let svc = MockConnectionService::ready();
        let views = svc.materialized_views().await.expect("views");
        let mut ids: Vec<_> = views.iter().map(|v| v.id.as_str()).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert!(n > 0);
        assert_eq!(ids.len(), n);
    }

    #[tokio::test]
    async fn empty_reaches_the_empty_state() {
        let svc = MockConnectionService::empty();
        assert!(AsyncState::from_value(Some(svc.materialized_views().await)).is_empty());
    }

    #[tokio::test]
    async fn erroring_reaches_the_error_state() {
        let svc = MockConnectionService::erroring();
        assert!(
            AsyncState::from_value(Some(svc.materialized_views().await))
                .error_message()
                .is_some()
        );
        assert!(svc.view_rows("matview_01", 0, 10).await.is_err());
        assert!(svc.refresh_history("matview_01").await.is_err());
    }

    #[tokio::test]
    async fn an_unknown_view_is_a_typed_error() {
        let svc = MockConnectionService::ready();
        assert!(matches!(
            svc.view_rows("matview_99", 0, 10).await,
            Err(StudioError::UnknownObject { .. })
        ));
    }

    #[tokio::test]
    async fn a_manual_refresh_shows_in_the_history() {
        let svc = MockConnectionService::ready();
        let before = svc
            .refresh_history("matview_01")
            .await
            .expect("history")
            .len();
        svc.refresh_view("matview_01").await.expect("refreshes");
        assert_eq!(
            svc.refresh_history("matview_01")
                .await
                .expect("history")
                .len(),
            before + 1
        );
    }
}
