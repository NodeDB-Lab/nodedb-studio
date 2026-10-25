//! The `IsEmpty` trait and its per-model impls, which tell
//! `AsyncState::from_value` when a loaded value maps to the Empty state.

use crate::models::admin::AdminReview;
use crate::models::admin_access::{AccessDirectory, SubjectAccess};
use crate::models::admin_audit::{AuditEvent, AuditFacets, AuditPage};
use crate::models::admin_backup::BackupOverview;
use crate::models::admin_config::{ConfigProposal, ConfigView};
use crate::models::admin_credential::{CredentialList, CredentialTest};
use crate::models::admin_policy::{PolicySet, PolicyTestResult};
use crate::models::array_data::{ArraySchema, SliceResult};
use crate::models::cluster::ClusterMembers;
use crate::models::cluster_activity::ActivityLog;
use crate::models::cluster_monitor::Telemetry;
use crate::models::cluster_place::Placement;
use crate::models::cluster_raft::RaftGroups;
use crate::models::designer::DesignSchema;
use crate::models::designer_change::ChangeReview;
use crate::models::feed_draft::FeedPreview;
use crate::models::geo_ops::Validity;
use crate::models::graph_records::EdgePage;
use crate::models::jobs::JobList;
use crate::models::matviews::ViewRows;
use crate::models::query_plan::QueryPlan;
use crate::models::records::{CollectionAccess, Record, RecordPage};
use crate::models::saved_query::QueryLibrary;
use crate::models::schema::CollectionSchema;
use crate::models::shell::{NavBadges, SessionInfo};
use crate::models::spatial::{Feature, RasterSlice, SpatialSetup, Viewport};
use crate::models::spatial_analyze::AnalysisResult;
use crate::models::sync::SyncDevices;
use crate::models::sync_activity::SyncLog;
use crate::models::sync_enroll::EnrollOptions;
use crate::models::sync_reject::{CompensationState, RejectedWrites};
use crate::models::sync_sub::{ChangePreview, Subscriptions};
use crate::models::timeseries::{ExploreResult, TimeCatalog};
use crate::models::topics::{MessageRange, ResetPreview};
use crate::models::transfer::ExportPreview;
use crate::models::ts_backfill::BackfillPreview;
use crate::models::ts_dashboard::Dashboard;
use crate::models::ts_lifecycle::Lifecycle;
use crate::models::workbench::{QueryRun, ResultSet, SchemaTree};

/// Anything that can report emptiness, so `from_value` can distinguish a
/// loaded-but-empty result (-> Empty) from a loaded-with-data result.
pub trait IsEmpty {
    fn is_empty(&self) -> bool;
}

impl<T> IsEmpty for Vec<T> {
    fn is_empty(&self) -> bool {
        Vec::is_empty(self)
    }
}

// Single-value seam reads have no "empty" shape: a fetched value is never
// "empty", it either arrived or it errored (`MockBehavior::Empty` folds into
// `Ready` for these — see `record_detail`'s precedent). Each impl is listed
// explicitly, deliberately not a blanket `impl<T> IsEmpty for T`, so a future
// single-value model must opt in here rather than silently inheriting a
// meaning that may not fit it.
impl IsEmpty for SessionInfo {
    fn is_empty(&self) -> bool {
        false
    }
}

/// A member list is empty when it lists no members. Its observation time
/// alone carries nothing to show.
impl IsEmpty for ClusterMembers {
    fn is_empty(&self) -> bool {
        self.members.is_empty()
    }
}

/// The Raft groups are empty when the cluster reports no group.
impl IsEmpty for RaftGroups {
    fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }
}

/// Placement is empty when no vShard is placed.
impl IsEmpty for Placement {
    fn is_empty(&self) -> bool {
        self.vshards.is_empty()
    }
}

/// Telemetry is empty when no node reports.
impl IsEmpty for Telemetry {
    fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

/// The activity log is empty when no event matches.
impl IsEmpty for ActivityLog {
    fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

/// The device list is empty when no device is registered.
impl IsEmpty for SyncDevices {
    fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }
}

impl IsEmpty for Subscriptions {
    fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl IsEmpty for RejectedWrites {
    fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

/// The directory is empty when it lists no principal.
impl IsEmpty for AccessDirectory {
    fn is_empty(&self) -> bool {
        self.principals.is_empty()
    }
}

/// A policy set is empty when the database has no policy.
impl IsEmpty for PolicySet {
    fn is_empty(&self) -> bool {
        self.policies.is_empty()
    }
}

/// A page is empty when no event matches the filters.
impl IsEmpty for AuditPage {
    fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

impl IsEmpty for BackupOverview {
    fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }
}

impl IsEmpty for ConfigView {
    fn is_empty(&self) -> bool {
        self.settings.is_empty()
    }
}

impl IsEmpty for CredentialList {
    fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// One subject, one event, one test, one review, and one proposal always
/// carry something: a missing one is an error.
macro_rules! never_empty {
    ($($t:ty),* $(,)?) => {
        $(impl IsEmpty for $t {
            fn is_empty(&self) -> bool {
                false
            }
        })*
    };
}

/// No proposed change has nothing to review.
impl IsEmpty for Option<ConfigProposal> {
    fn is_empty(&self) -> bool {
        self.is_none()
    }
}

/// A subject that is not chosen yet has nothing to show.
impl IsEmpty for Option<SubjectAccess> {
    fn is_empty(&self) -> bool {
        self.is_none()
    }
}

never_empty!(
    SubjectAccess,
    AuditEvent,
    AuditFacets,
    PolicyTestResult,
    AdminReview,
    ConfigProposal,
    CredentialTest,
);

impl IsEmpty for SyncLog {
    fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

/// A repair state, a preview, and the options always carry something.
impl IsEmpty for CompensationState {
    fn is_empty(&self) -> bool {
        false
    }
}

impl IsEmpty for ChangePreview {
    fn is_empty(&self) -> bool {
        false
    }
}

impl IsEmpty for EnrollOptions {
    fn is_empty(&self) -> bool {
        false
    }
}

/// A job list is empty when the server lists no jobs.
impl IsEmpty for JobList {
    fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }
}

/// A view that holds no rows has nothing to page.
impl IsEmpty for ViewRows {
    fn is_empty(&self) -> bool {
        self.total == 0
    }
}

/// A bounded read with no messages still loaded: the range says what it
/// covered and the retained bounds.
impl IsEmpty for MessageRange {
    fn is_empty(&self) -> bool {
        false
    }
}

impl IsEmpty for ResetPreview {
    fn is_empty(&self) -> bool {
        false
    }
}

impl IsEmpty for FeedPreview {
    fn is_empty(&self) -> bool {
        false
    }
}

impl IsEmpty for NavBadges {
    fn is_empty(&self) -> bool {
        false
    }
}

/// A record read is one value: it exists or it is `RecordMissing`.
impl IsEmpty for Record {
    fn is_empty(&self) -> bool {
        false
    }
}

/// A page with no rows still loaded: the Explorer states why it is empty
/// (a filter, a range) and offers the way out, which `AsyncView`'s one
/// message cannot.
impl IsEmpty for RecordPage {
    fn is_empty(&self) -> bool {
        false
    }
}

impl IsEmpty for EdgePage {
    fn is_empty(&self) -> bool {
        false
    }
}

impl IsEmpty for CollectionSchema {
    fn is_empty(&self) -> bool {
        false
    }
}

impl IsEmpty for CollectionAccess {
    fn is_empty(&self) -> bool {
        false
    }
}

/// A connection with no geometry source and no georeferenced array has
/// nothing for the Spatial screens to open.
impl IsEmpty for SpatialSetup {
    fn is_empty(&self) -> bool {
        self.sources.is_empty() && self.rasters.is_empty()
    }
}

/// A viewport with no features still loaded: the map says the box is empty.
impl IsEmpty for Viewport {
    fn is_empty(&self) -> bool {
        false
    }
}

impl IsEmpty for Feature {
    fn is_empty(&self) -> bool {
        false
    }
}

/// A feature read with nothing selected is a loaded absence, not an empty
/// result.
impl IsEmpty for Option<Feature> {
    fn is_empty(&self) -> bool {
        false
    }
}

impl IsEmpty for Validity {
    fn is_empty(&self) -> bool {
        false
    }
}

/// An analysis with no rows still ran: the table says nothing matched.
impl IsEmpty for AnalysisResult {
    fn is_empty(&self) -> bool {
        false
    }
}

impl IsEmpty for RasterSlice {
    fn is_empty(&self) -> bool {
        false
    }
}

impl IsEmpty for ArraySchema {
    fn is_empty(&self) -> bool {
        false
    }
}

impl IsEmpty for SliceResult {
    fn is_empty(&self) -> bool {
        false
    }
}

impl IsEmpty for ExportPreview {
    fn is_empty(&self) -> bool {
        false
    }
}

impl IsEmpty for QueryPlan {
    fn is_empty(&self) -> bool {
        false
    }
}

// `ResultSet` is also a single fetched value, but unlike the four above it
// wraps a list of rows whose emptiness IS a real, common outcome: "your query
// returned no rows". Folding `MockBehavior::Empty` into `Ready` for it would
// make `AsyncState::Empty` unreachable for the query pane, so it reports its
// own emptiness instead of the
// blanket `false` above (see `apply_one_or_empty` in `mock_behavior`, which
// gives the mock seam methods a way to actually deliver an empty payload).
impl IsEmpty for ResultSet {
    fn is_empty(&self) -> bool {
        self.rows().is_empty()
    }
}

/// A run with no rows still ran: the Results panel states that, with the
/// columns, rather than the generic empty message.
impl IsEmpty for QueryRun {
    fn is_empty(&self) -> bool {
        false
    }
}

/// A schema with no collections has nothing to draw. Its references and
/// off-canvas names alone carry nothing to show.
impl IsEmpty for DesignSchema {
    fn is_empty(&self) -> bool {
        self.collections.is_empty()
    }
}

impl IsEmpty for ChangeReview {
    fn is_empty(&self) -> bool {
        false
    }
}

impl IsEmpty for SchemaTree {
    fn is_empty(&self) -> bool {
        self.tables.is_empty() && self.views.is_empty()
    }
}

/// A result is empty when no series holds a point. The range and the
/// counts of rows read still say what was asked.
impl IsEmpty for ExploreResult {
    fn is_empty(&self) -> bool {
        ExploreResult::is_empty(self)
    }
}

/// A catalog is empty when the database offers no collection.
impl IsEmpty for TimeCatalog {
    fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }
}

/// A dashboard document is never an empty state: an empty one is a draft.
impl IsEmpty for Dashboard {
    fn is_empty(&self) -> bool {
        false
    }
}

/// A lifecycle always has its raw tier.
impl IsEmpty for Lifecycle {
    fn is_empty(&self) -> bool {
        false
    }
}

impl IsEmpty for BackfillPreview {
    fn is_empty(&self) -> bool {
        self.buckets.is_empty()
    }
}

impl IsEmpty for QueryLibrary {
    fn is_empty(&self) -> bool {
        self.queries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::explorer::RecordRow;
    use crate::services::async_state::AsyncState;

    #[test]
    fn single_value_seam_models_are_loaded_not_empty() {
        // Before their `IsEmpty` impls existed, these four single-value seam
        // models could not satisfy `AsyncState<T>::from_value`'s `T: IsEmpty`
        // bound at all, so they could never be rendered through `AsyncView`.
        // Each must map a fetched value straight to `Loaded`, never `Empty`:
        // none of the four has a meaningful "empty" shape.
        let session_info = AsyncState::from_value(Some(Ok(SessionInfo {
            server_version: String::new(),
        })));
        assert!(matches!(session_info, AsyncState::Loaded(_)));

        let nav_badges = AsyncState::from_value(Some(Ok(NavBadges { query: 0 })));
        assert!(matches!(nav_badges, AsyncState::Loaded(_)));

        let record = AsyncState::from_value(Some(Ok(Record::new("r1", 1, Vec::new()))));
        assert!(matches!(record, AsyncState::Loaded(_)));

        let page = AsyncState::from_value(Some(Ok(RecordPage {
            rows: Vec::new(),
            total: 0,
            observed_at: 0,
        })));
        assert!(matches!(page, AsyncState::Loaded(_)));

        let query_plan = AsyncState::from_value(Some(Ok(QueryPlan {
            mode: crate::models::query_plan::ExplainMode::Explain,
            operators: Vec::new(),
            elapsed_ms: None,
        })));
        assert!(matches!(query_plan, AsyncState::Loaded(_)));
    }

    #[test]
    fn zero_row_result_set_is_empty() {
        // Unlike the four single-value models above, `ResultSet` wraps a
        // list: a query that returns zero rows is the most common
        // non-error workbench outcome and must reach `AsyncState::Empty`,
        // not `Loaded` with an empty table.
        let result_set = AsyncState::from_value(Some(Ok(ResultSet::new(
            vec!["id".to_string()],
            Vec::new(),
            3,
            "0 rows".to_string(),
        )
        .expect("rectangular"))));
        assert!(matches!(result_set, AsyncState::Empty));
    }

    #[test]
    fn non_empty_result_set_is_loaded() {
        let result_set = AsyncState::from_value(Some(Ok(ResultSet::new(
            vec!["id".to_string()],
            vec![RecordRow {
                id: "1".to_string(),
                cells: vec!["1".to_string()],
            }],
            3,
            "1 row".to_string(),
        )
        .expect("rectangular"))));
        assert!(matches!(result_set, AsyncState::Loaded(_)));
    }
}
