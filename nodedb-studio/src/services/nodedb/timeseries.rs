//! Timeseries reads and writes. Not wired to a server yet.

use async_trait::async_trait;

use crate::models::timeseries::{ExploreQuery, ExploreResult, TimeCatalog};
use crate::models::ts_backfill::{BackfillPreview, BackfillRun, BackfillSpec};
use crate::models::ts_dashboard::{Dashboard, DashboardSummary};
use crate::models::ts_lifecycle::{ChangeReview, Lifecycle, LifecycleChange, Receipt, RollupRun};
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;
use crate::services::timeseries_data::TimeseriesData;

#[async_trait(?Send)]
impl TimeseriesData for NodeDbConnectionService {
    async fn time_sources(&self) -> Result<TimeCatalog, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn explore(&self, _query: &ExploreQuery) -> Result<ExploreResult, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn dashboards(&self) -> Result<Vec<DashboardSummary>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn dashboard(&self, _id: &str) -> Result<Dashboard, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn save_dashboard(&self, _doc: &Dashboard) -> Result<Dashboard, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn save_dashboard_as(
        &self,
        _name: &str,
        _doc: &Dashboard,
    ) -> Result<Dashboard, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn rename_dashboard(
        &self,
        _id: &str,
        _revision: u64,
        _name: &str,
    ) -> Result<Dashboard, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn duplicate_dashboard(&self, _id: &str) -> Result<Dashboard, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn delete_dashboard(&self, _id: &str) -> Result<(), StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn lifecycle(&self, _collection: &str) -> Result<Lifecycle, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn review_lifecycle(
        &self,
        _collection: &str,
        _changes: &[LifecycleChange],
    ) -> Result<ChangeReview, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn apply_lifecycle(
        &self,
        _collection: &str,
        _changes: &[LifecycleChange],
        _reviewed: u64,
    ) -> Result<Receipt, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn rollup_runs(
        &self,
        _collection: &str,
        _rollup: &str,
    ) -> Result<Vec<RollupRun>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn refresh_rollup(
        &self,
        _collection: &str,
        _rollup: &str,
    ) -> Result<Receipt, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn backfill_preview(&self, _spec: &BackfillSpec) -> Result<BackfillPreview, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn start_backfill(
        &self,
        _spec: &BackfillSpec,
        _preview_revision: &str,
        _key: &str,
    ) -> Result<BackfillRun, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn resume_backfill(&self, _id: &str) -> Result<BackfillRun, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn backfill_runs(&self, _target: Option<&str>) -> Result<Vec<BackfillRun>, StudioError> {
        Err(StudioError::NotConnected)
    }
}
