//! Timeseries reads and writes. Documents, lifecycle changes, and backfill
//! jobs land in the service's in-memory store and every later read sees them.

use async_trait::async_trait;
use nodedb_client::NodeDbError;
use nodedb_types::Value;

use crate::data::mock::{
    self, BackfillError, DashError, ExploreError, LifecycleError, backfill_preview,
};
use crate::models::timeseries::{ExploreQuery, ExploreResult, TimeCatalog};
use crate::models::ts_backfill::{BackfillPreview, BackfillRun, BackfillSpec};
use crate::models::ts_dashboard::{Dashboard, DashboardSummary};
use crate::models::ts_lifecycle::{ChangeReview, Lifecycle, LifecycleChange, Receipt, RollupRun};
use crate::services::error::StudioError;
use crate::services::mock::MockConnectionService;
use crate::services::mock_behavior::{MockBehavior, apply_one_result, apply_result, apply_write};
use crate::services::timeseries_data::TimeseriesData;

fn dash_error(e: DashError) -> StudioError {
    match e {
        DashError::Unknown(id) => StudioError::UnknownObject {
            kind: "dashboard",
            id,
        },
        DashError::Stale {
            name,
            expected,
            stored,
        } => StudioError::StaleRevision {
            object: format!("dashboard {name}"),
            expected,
            stored,
        },
        DashError::NameTaken(name) => StudioError::Rejected {
            reason: format!("A dashboard named `{name}` already exists. Choose another name."),
        },
        DashError::Invalid(reason) => StudioError::Rejected {
            reason: reason.to_string(),
        },
    }
}

fn explore_error(e: ExploreError) -> StudioError {
    match e {
        ExploreError::UnknownCollection(c) => {
            StudioError::NotFound(NodeDbError::collection_not_found(&c))
        }
        ExploreError::Invalid(issues) => StudioError::Rejected {
            reason: issues
                .into_iter()
                .map(|i| i.message)
                .collect::<Vec<_>>()
                .join(" "),
        },
    }
}

fn lifecycle_error(collection: &str, e: LifecycleError) -> StudioError {
    match e {
        LifecycleError::Stale { expected, stored } => StudioError::StaleRevision {
            object: format!("{collection} lifecycle"),
            expected,
            stored,
        },
        LifecycleError::Blocked(why) => StudioError::Rejected {
            reason: why.join(" "),
        },
    }
}

fn backfill_error(e: BackfillError) -> StudioError {
    match e {
        BackfillError::Invalid(reason) => StudioError::Rejected { reason },
        BackfillError::UnknownRun(id) => StudioError::UnknownObject {
            kind: "backfill run",
            id,
        },
        BackfillError::NameInUse(name) => StudioError::Rejected {
            reason: format!("A backfill job named `{name}` already exists. Choose another name."),
        },
        BackfillError::StalePreview { .. } => StudioError::Rejected {
            reason: "The preview no longer matches the job. Run the dry run again.".to_string(),
        },
    }
}

impl MockConnectionService {
    /// Only `sales` has a lifecycle fixture.
    fn lifecycle_scope(&self, collection: &str) -> Result<(), StudioError> {
        if collection == "sales" {
            Ok(())
        } else {
            Err(StudioError::NotFound(NodeDbError::collection_not_found(
                collection,
            )))
        }
    }

    /// The event times of the stored `sales` rows, this session's writes
    /// included. A review counts the rows a retention change would expire.
    fn raw_times(&self) -> Vec<i64> {
        mock::all_records("sales", &self.records.borrow())
            .unwrap_or_default()
            .iter()
            .filter_map(|r| match r.get("sold_at") {
                Some(Value::DateTime(t)) => Some(t.unix_secs()),
                _ => None,
            })
            .collect()
    }
}

#[async_trait(?Send)]
impl TimeseriesData for MockConnectionService {
    async fn time_sources(&self) -> Result<TimeCatalog, StudioError> {
        let mut catalog = TimeCatalog {
            observed_at: mock::observed_at(),
            sources: mock::time_sources(),
        };
        // `Empty` keeps the clock but offers no collection.
        if self.behavior == MockBehavior::Empty {
            catalog.sources.clear();
        }
        apply_one_result(self.behavior, Ok(catalog)).await
    }

    async fn explore(&self, query: &ExploreQuery) -> Result<ExploreResult, StudioError> {
        let mut result = mock::explore(query, &self.records.borrow()).map_err(explore_error);
        // `Empty` answers every query with no series.
        if self.behavior == MockBehavior::Empty
            && let Ok(r) = result.as_mut()
        {
            r.series.clear();
        }
        apply_one_result(self.behavior, result).await
    }

    async fn dashboards(&self) -> Result<Vec<DashboardSummary>, StudioError> {
        let list = self.timeseries.borrow().dashboards.list();
        apply_result(self.behavior, Ok(list)).await
    }

    async fn dashboard(&self, id: &str) -> Result<Dashboard, StudioError> {
        let read = self
            .timeseries
            .borrow()
            .dashboards
            .get(id)
            .map_err(dash_error);
        apply_one_result(self.behavior, read).await
    }

    async fn save_dashboard(&self, doc: &Dashboard) -> Result<Dashboard, StudioError> {
        apply_write(self.behavior, || {
            self.timeseries
                .borrow_mut()
                .dashboards
                .save(doc)
                .map_err(dash_error)
        })
        .await
    }

    async fn save_dashboard_as(
        &self,
        name: &str,
        doc: &Dashboard,
    ) -> Result<Dashboard, StudioError> {
        apply_write(self.behavior, || {
            self.timeseries
                .borrow_mut()
                .dashboards
                .save_as(name, doc)
                .map_err(dash_error)
        })
        .await
    }

    async fn rename_dashboard(
        &self,
        id: &str,
        revision: u64,
        name: &str,
    ) -> Result<Dashboard, StudioError> {
        apply_write(self.behavior, || {
            self.timeseries
                .borrow_mut()
                .dashboards
                .rename(id, revision, name)
                .map_err(dash_error)
        })
        .await
    }

    async fn duplicate_dashboard(&self, id: &str) -> Result<Dashboard, StudioError> {
        apply_write(self.behavior, || {
            self.timeseries
                .borrow_mut()
                .dashboards
                .duplicate(id)
                .map_err(dash_error)
        })
        .await
    }

    async fn delete_dashboard(&self, id: &str) -> Result<(), StudioError> {
        apply_write(self.behavior, || {
            self.timeseries
                .borrow_mut()
                .dashboards
                .delete(id)
                .map_err(dash_error)
        })
        .await
    }

    async fn lifecycle(&self, collection: &str) -> Result<Lifecycle, StudioError> {
        let read = self
            .lifecycle_scope(collection)
            .map(|()| self.timeseries.borrow().lifecycle.lifecycle.clone());
        apply_one_result(self.behavior, read).await
    }

    async fn review_lifecycle(
        &self,
        collection: &str,
        changes: &[LifecycleChange],
    ) -> Result<ChangeReview, StudioError> {
        let read = self.lifecycle_scope(collection).map(|()| {
            let t = self.timeseries.borrow();
            t.lifecycle.review(
                changes,
                &self.raw_times(),
                t.runs.all(),
                mock::observed_at(),
            )
        });
        apply_one_result(self.behavior, read).await
    }

    async fn apply_lifecycle(
        &self,
        collection: &str,
        changes: &[LifecycleChange],
        reviewed: u64,
    ) -> Result<Receipt, StudioError> {
        apply_write(self.behavior, || {
            self.lifecycle_scope(collection)?;
            let times = self.raw_times();
            let mut t = self.timeseries.borrow_mut();
            let operation = t.next_operation();
            let jobs = t.runs.all().to_vec();
            t.lifecycle
                .apply(
                    changes,
                    reviewed,
                    &times,
                    &jobs,
                    mock::observed_at(),
                    operation,
                )
                .map_err(|e| lifecycle_error(collection, e))
        })
        .await
    }

    async fn rollup_runs(
        &self,
        collection: &str,
        rollup: &str,
    ) -> Result<Vec<RollupRun>, StudioError> {
        let read = self
            .lifecycle_scope(collection)
            .map(|()| self.timeseries.borrow().lifecycle.runs(rollup));
        apply_result(self.behavior, read).await
    }

    async fn refresh_rollup(&self, collection: &str, rollup: &str) -> Result<Receipt, StudioError> {
        apply_write(self.behavior, || {
            self.lifecycle_scope(collection)?;
            let mut t = self.timeseries.borrow_mut();
            let operation = t.next_operation();
            t.lifecycle
                .queue_refresh(rollup, operation.clone(), mock::observed_at())
                .map_err(|e| lifecycle_error(collection, e))?;
            Ok(Receipt {
                operation,
                applied: 1,
                revision: t.lifecycle.lifecycle.revision,
            })
        })
        .await
    }

    async fn backfill_preview(&self, spec: &BackfillSpec) -> Result<BackfillPreview, StudioError> {
        let read = {
            let t = self.timeseries.borrow();
            match spec.issues().into_iter().next() {
                Some(reason) => Err(StudioError::Rejected { reason }),
                None => Ok(backfill_preview(
                    spec,
                    &t.lifecycle.lifecycle,
                    mock::observed_at(),
                )),
            }
        };
        apply_one_result(self.behavior, read).await
    }

    async fn start_backfill(
        &self,
        spec: &BackfillSpec,
        preview_revision: &str,
        key: &str,
    ) -> Result<BackfillRun, StudioError> {
        apply_write(self.behavior, || {
            let mut t = self.timeseries.borrow_mut();
            let lifecycle = t.lifecycle.lifecycle.clone();
            t.runs
                .start(spec, preview_revision, key, &lifecycle, mock::observed_at())
                .map_err(backfill_error)
        })
        .await
    }

    async fn resume_backfill(&self, id: &str) -> Result<BackfillRun, StudioError> {
        apply_write(self.behavior, || {
            self.timeseries
                .borrow_mut()
                .runs
                .resume(id, mock::observed_at())
                .map_err(backfill_error)
        })
        .await
    }

    async fn backfill_runs(&self, target: Option<&str>) -> Result<Vec<BackfillRun>, StudioError> {
        let runs = self.timeseries.borrow().runs.runs(target);
        apply_result(self.behavior, Ok(runs)).await
    }
}
