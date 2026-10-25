//! Timeseries reads and writes at the backend seam: the collections a
//! database offers, exploration queries, dashboard documents, lifecycle
//! changes, and backfill jobs.
//!
//! Every method scopes to the active session's database. A write that names
//! a revision is refused when the stored revision moved
//! (`StudioError::StaleRevision`): nothing is overwritten.

use async_trait::async_trait;

use crate::models::timeseries::{ExploreQuery, ExploreResult, TimeCatalog};
use crate::models::ts_backfill::{BackfillPreview, BackfillRun, BackfillSpec};
use crate::models::ts_dashboard::{Dashboard, DashboardSummary};
use crate::models::ts_lifecycle::{ChangeReview, Lifecycle, LifecycleChange, Receipt, RollupRun};
use crate::services::error::StudioError;

#[async_trait(?Send)]
pub trait TimeseriesData {
    /// Every collection of the database, with whether it can be explored
    /// over time and why not, and the instant the catalog was read.
    async fn time_sources(&self) -> Result<TimeCatalog, StudioError>;

    /// Run one query. A query the catalog refuses is `StudioError::Rejected`.
    async fn explore(&self, query: &ExploreQuery) -> Result<ExploreResult, StudioError>;

    /// The saved dashboards of the workspace, for the database in scope.
    async fn dashboards(&self) -> Result<Vec<DashboardSummary>, StudioError>;

    async fn dashboard(&self, id: &str) -> Result<Dashboard, StudioError>;

    /// Store a document over itself. It must carry an identity and the
    /// revision it read.
    async fn save_dashboard(&self, doc: &Dashboard) -> Result<Dashboard, StudioError>;

    /// Store a copy under `name`. A name another document holds is refused.
    async fn save_dashboard_as(
        &self,
        name: &str,
        doc: &Dashboard,
    ) -> Result<Dashboard, StudioError>;

    async fn rename_dashboard(
        &self,
        id: &str,
        revision: u64,
        name: &str,
    ) -> Result<Dashboard, StudioError>;

    async fn duplicate_dashboard(&self, id: &str) -> Result<Dashboard, StudioError>;

    async fn delete_dashboard(&self, id: &str) -> Result<(), StudioError>;

    /// Retention, rollups, and the late-data policy of a timeseries
    /// collection. Another collection is `StudioError::NotFound`.
    async fn lifecycle(&self, collection: &str) -> Result<Lifecycle, StudioError>;

    /// What `changes` would do, bound to the revision read.
    async fn review_lifecycle(
        &self,
        collection: &str,
        changes: &[LifecycleChange],
    ) -> Result<ChangeReview, StudioError>;

    /// Apply reviewed changes. `reviewed` is the review's revision.
    async fn apply_lifecycle(
        &self,
        collection: &str,
        changes: &[LifecycleChange],
        reviewed: u64,
    ) -> Result<Receipt, StudioError>;

    /// Refreshes queued for one rollup, newest first.
    async fn rollup_runs(
        &self,
        collection: &str,
        rollup: &str,
    ) -> Result<Vec<RollupRun>, StudioError>;

    /// Queue a refresh of one rollup. The receipt names the operation.
    async fn refresh_rollup(&self, collection: &str, rollup: &str) -> Result<Receipt, StudioError>;

    /// A dry run: the buckets a backfill would replace. Changes nothing.
    async fn backfill_preview(&self, spec: &BackfillSpec) -> Result<BackfillPreview, StudioError>;

    /// Start a job under a reviewed preview. A repeat of `key` is the same
    /// job, never a second one.
    async fn start_backfill(
        &self,
        spec: &BackfillSpec,
        preview_revision: &str,
        key: &str,
    ) -> Result<BackfillRun, StudioError>;

    /// Continue an interrupted job from its checkpoint.
    async fn resume_backfill(&self, id: &str) -> Result<BackfillRun, StudioError>;

    /// Backfill jobs, newest first. `None` lists every target.
    async fn backfill_runs(&self, target: Option<&str>) -> Result<Vec<BackfillRun>, StudioError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::format::epoch;
    use crate::models::records::Filter;
    use crate::models::timeseries::{Aggregate, Bucket, GapPolicy, Measure, TimeSpan};
    use crate::models::ts_backfill::{ConflictPolicy, RunState, WriteMode};
    use crate::models::ts_lifecycle::{
        LateWrites, Retention, RetentionDraft, RollupFn, RollupOutput,
    };
    use crate::services::async_state::AsyncState;
    use crate::services::mock::MockConnectionService;

    fn query() -> ExploreQuery {
        ExploreQuery {
            collection: "sales".into(),
            time_field: "sold_at".into(),
            measure: Measure::of(Aggregate::Sum, "revenue"),
            filters: vec![Filter {
                field: "currency".into(),
                value: "USD".into(),
            }],
            bucket: Bucket::Day1,
            group_by: Some("store_id".into()),
            gap: GapPolicy::Missing,
            span: TimeSpan::new(epoch(2026, 6, 7, 0, 0, 0), epoch(2026, 6, 14, 0, 0, 0)),
        }
    }

    fn spec() -> BackfillSpec {
        BackfillSpec {
            target: "sales_daily".into(),
            bucket: Bucket::Day1,
            source: "analytics.sales".into(),
            time_field: "sold_at".into(),
            range: TimeSpan::new(epoch(2026, 6, 1, 0, 0, 0), epoch(2026, 6, 8, 0, 0, 0)),
            aggregates: vec![RollupOutput {
                func: RollupFn::Sum,
                column: Some("revenue".into()),
                alias: "revenue".into(),
            }],
            currency: Some("USD".into()),
            late: LateWrites::RecomputeAffected,
            job_name: "backfill104".into(),
            description: String::new(),
            mode: WriteMode::ReplaceAffected,
            conflict: ConflictPolicy::ReplaceAffected,
            target_key: "day + store_id".into(),
            checkpoint: true,
        }
    }

    #[tokio::test]
    async fn sources_have_unique_names_and_the_sales_collection_is_eligible() {
        let svc = MockConnectionService::ready();
        let sources = svc.time_sources().await.expect("sources").sources;
        let mut names: Vec<&str> = sources.iter().map(|s| s.collection.as_str()).collect();
        let n = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(n, names.len());
        assert!(
            sources
                .iter()
                .any(|s| s.collection == "sales" && s.eligible())
        );
    }

    #[tokio::test]
    async fn explore_runs_a_valid_query_and_rejects_an_invalid_one() {
        let svc = MockConnectionService::ready();
        let r = svc.explore(&query()).await.expect("result");
        assert!(!r.is_empty());
        let mut bad = query();
        bad.filters.clear();
        assert!(matches!(
            svc.explore(&bad).await,
            Err(StudioError::Rejected { .. })
        ));
        bad.collection = "nope".into();
        assert!(matches!(
            svc.explore(&bad).await,
            Err(StudioError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn dashboards_have_unique_ids_and_survive_a_save_round_trip() {
        let svc = MockConnectionService::ready();
        let list = svc.dashboards().await.expect("list");
        let mut ids: Vec<&str> = list.iter().map(|d| d.id.as_str()).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(n, ids.len());
        let mut doc = svc.dashboard("dash-1").await.expect("doc");
        doc.panels[0].title = "Revenue".into();
        let saved = svc.save_dashboard(&doc).await.expect("saved");
        assert_eq!(saved.revision, doc.revision + 1);
        let again = svc.dashboard("dash-1").await.expect("doc");
        assert_eq!(again, saved);
        // The old revision is now stale.
        assert!(matches!(
            svc.save_dashboard(&doc).await,
            Err(StudioError::StaleRevision { .. })
        ));
    }

    #[tokio::test]
    async fn save_as_duplicate_rename_and_delete_change_the_library() {
        let svc = MockConnectionService::ready();
        let doc = svc.dashboard("dash-1").await.expect("doc");
        let copy = svc.save_dashboard_as("Copy", &doc).await;
        assert!(copy.is_ok());
        assert!(matches!(
            svc.save_dashboard_as("copy", &doc).await,
            Err(StudioError::Rejected { .. })
        ));
        let dup = svc.duplicate_dashboard("dash-2").await.expect("dup");
        assert_eq!(dup.name, "Order volume copy");
        let id = dup.id.clone().expect("id");
        let renamed = svc
            .rename_dashboard(&id, dup.revision, "Orders")
            .await
            .expect("renamed");
        assert_eq!(renamed.name, "Orders");
        svc.delete_dashboard(&id).await.expect("deleted");
        assert!(matches!(
            svc.dashboard(&id).await,
            Err(StudioError::UnknownObject { .. })
        ));
    }

    #[tokio::test]
    async fn lifecycle_reviews_then_applies_a_retention_change() {
        let svc = MockConnectionService::ready();
        let l = svc.lifecycle("sales").await.expect("lifecycle");
        assert!(matches!(
            svc.lifecycle("users").await,
            Err(StudioError::NotFound(_))
        ));
        let mut d = RetentionDraft::from(&l);
        d.raw_keep = Retention::Days(180);
        let changes = [LifecycleChange::Retention(d)];
        let review = svc
            .review_lifecycle("sales", &changes)
            .await
            .expect("review");
        assert!(review.blocked.is_empty());
        let receipt = svc
            .apply_lifecycle("sales", &changes, review.revision)
            .await
            .expect("applied");
        assert!(receipt.operation.starts_with("op-"));
        assert_eq!(
            svc.lifecycle("sales").await.expect("lifecycle").raw_keep,
            Retention::Days(180)
        );
        assert!(matches!(
            svc.apply_lifecycle("sales", &changes, review.revision)
                .await,
            Err(StudioError::StaleRevision { .. })
        ));
    }

    #[tokio::test]
    async fn a_refresh_is_queued_and_listed_with_its_operation() {
        let svc = MockConnectionService::ready();
        let r = svc
            .refresh_rollup("sales", "sales_hourly")
            .await
            .expect("queued");
        let runs = svc
            .rollup_runs("sales", "sales_hourly")
            .await
            .expect("runs");
        assert_eq!(runs[0].operation, r.operation);
    }

    #[tokio::test]
    async fn a_backfill_previews_runs_once_and_resumes() {
        let svc = MockConnectionService::ready();
        let s = spec();
        let p = svc.backfill_preview(&s).await.expect("preview");
        assert_eq!(p.scanned_rows, 12_040);
        let key = s.idempotency_key(&p.revision);
        let run = svc
            .start_backfill(&s, &p.revision, &key)
            .await
            .expect("run");
        let again = svc
            .start_backfill(&s, &p.revision, &key)
            .await
            .expect("same");
        assert_eq!(run, again);
        let all = svc.backfill_runs(Some("sales_daily")).await.expect("runs");
        assert_eq!(all.iter().filter(|r| r.id == "backfill104").count(), 1);
        let resumed = svc.resume_backfill("backfill102").await.expect("resumed");
        assert_eq!(resumed.state, RunState::Completed);
        let ids: Vec<&str> = all.iter().map(|r| r.id.as_str()).collect();
        let mut dedup = ids.clone();
        dedup.sort_unstable();
        dedup.dedup();
        assert_eq!(ids.len(), dedup.len(), "run ids are unique keys");
    }

    #[tokio::test]
    async fn empty_behaviour_reaches_the_empty_state() {
        let svc = MockConnectionService::empty();
        assert!(AsyncState::from_value(Some(svc.time_sources().await)).is_empty());
        assert!(AsyncState::from_value(Some(svc.dashboards().await)).is_empty());
        assert!(AsyncState::from_value(Some(svc.backfill_runs(None).await)).is_empty());
        assert!(AsyncState::from_value(Some(svc.explore(&query()).await)).is_empty());
    }

    #[tokio::test]
    async fn erroring_behaviour_reaches_the_error_state() {
        let svc = MockConnectionService::erroring();
        assert!(svc.time_sources().await.is_err());
        assert!(svc.explore(&query()).await.is_err());
        assert!(svc.dashboards().await.is_err());
        assert!(svc.dashboard("dash-1").await.is_err());
        assert!(svc.lifecycle("sales").await.is_err());
        assert!(svc.backfill_preview(&spec()).await.is_err());
        assert!(svc.backfill_runs(None).await.is_err());
        let doc = MockConnectionService::ready()
            .dashboard("dash-1")
            .await
            .expect("doc");
        assert!(svc.save_dashboard(&doc).await.is_err());
    }
}
