//! The backend seam.
//!
//! Everything the UI needs from "the outside world" goes through this trait.
//! Today the only implementor is `MockConnectionService`, reading
//! `crate::data::mock`. When NodeDB's real client lands, a second implementor
//! wraps it here — and async (`use_resource`) is introduced at this boundary,
//! not sprinkled through the views.

use std::cell::Cell;
use std::rc::Rc;

use async_trait::async_trait;

use crate::data::mock;
use crate::models::admin::{AuditEntry, ClusterNode, RaftGroup, RlsPolicy, ShardRange, UserRow};
use crate::models::cdc::CdcRow;
use crate::models::explorer::{CollectionGroup, RecordDetail, RecordRow};
use crate::models::notification::Notification;
use crate::models::shell::{NavBadges, SessionInfo};
use crate::models::streams::{
    MaterializedView, NotifyChannel, NotifyMessage, ScheduledJob, StreamSession, Topic,
};
use crate::models::viewers::{
    FtsHit, SeriesPoint, SpatialFeature, SubGraph, SyncPeer, VectorPoint,
};
use crate::models::workbench::{QueryPlan, ResultSet, SchemaNode};
use crate::services::admin_data::AdminData;
use crate::services::error::StudioError;
use crate::services::explorer_data::ExplorerData;
use crate::services::mock_behavior::{MockBehavior, apply, apply_one, apply_one_or_empty};
use crate::services::streams_data::{StreamsData, cdc_rows_from_mock};
use crate::services::viewers_data::ViewersData;
use crate::services::workbench_data::WorkbenchData;
use crate::state::connection::ActiveConnection;
use crate::state::connections_registry::{Credentials, SavedConnection};

/// Async because the real client talks to NodeDB over the network. The Dioxus
/// runtime is single-threaded, so `?Send` is correct (and `use_resource` has no
/// `Send` bound). Every method returns `Result<_, StudioError>`: the mock can
/// only fail `connect` (unknown/offline name -> `NotConnected`), but the real
/// impl surfaces real failures through the same channel.
#[async_trait(?Send)]
pub trait ConnectionService {
    /// The saved-connection registry backing the Connection Manager.
    async fn list_connections(&self) -> Result<Vec<SavedConnection>, StudioError>;

    /// The full notification feed (capability gating happens at render time).
    async fn notifications(&self) -> Result<Vec<Notification>, StudioError>;

    /// Open a session by saved-connection name using an explicit identity.
    /// `StudioError::MissingUsername` if the username is blank;
    /// `StudioError::NotConnected` if the name is unknown or offline.
    async fn connect(
        &self,
        name: &str,
        creds: &Credentials,
    ) -> Result<ActiveConnection, StudioError>;

    /// Mark every notification read. A seam write: the real client persists this
    /// server-side; the mock persists it in-process so the unread badge does not
    /// revert on reload (POP-03).
    async fn mark_all_read(&self) -> Result<(), StudioError>;

    /// Badge counts for the nav rail's Query and Streams entries.
    #[allow(dead_code)] // SEAM-UNWIRED
    async fn nav_badges(&self) -> Result<NavBadges, StudioError>;

    /// The active session summary shown in the statusbar.
    #[allow(dead_code)] // SEAM-UNWIRED
    async fn session_info(&self) -> Result<SessionInfo, StudioError>;

    /// All databases visible on the active connection.
    #[allow(dead_code)] // SEAM-UNWIRED
    async fn databases(&self) -> Result<Vec<String>, StudioError>;
}

/// Hardcoded implementation used by the skeleton. Data is identical to before;
/// the methods are merely `async` now (and resolve instantly).
///
/// `all_read` uses `Rc<Cell<bool>>` for interior mutability so mark_all_read
/// persists across subsequent `notifications()` calls without blocking the
/// single-threaded Dioxus runtime. `Copy` is intentionally dropped — `Rc` is
/// not `Copy`.
#[derive(Debug, Clone, Default)]
pub struct MockConnectionService {
    behavior: MockBehavior,
    all_read: Rc<Cell<bool>>,
    /// Offset the next `cdc_batch` reads from. Only `commit_stream_offsets`
    /// advances it, mirroring the server: reads are idempotent.
    cdc_committed: Rc<Cell<usize>>,
    /// How far the most recent `cdc_batch` read. Commit promotes this into
    /// `cdc_committed`.
    cdc_read_end: Rc<Cell<usize>>,
}

// Constructors are public API for demos and tests; not all are used in the app binary.
impl MockConnectionService {
    /// Rich fixtures (the default).
    pub fn ready() -> Self {
        Self {
            behavior: MockBehavior::Ready,
            all_read: Rc::default(),
            cdc_committed: Rc::default(),
            cdc_read_end: Rc::default(),
        }
    }
    /// Every read returns an empty collection.
    #[allow(dead_code)]
    pub fn empty() -> Self {
        Self {
            behavior: MockBehavior::Empty,
            all_read: Rc::default(),
            cdc_committed: Rc::default(),
            cdc_read_end: Rc::default(),
        }
    }
    /// Every read fails with a retriable server error.
    #[allow(dead_code)]
    pub fn erroring() -> Self {
        Self {
            behavior: MockBehavior::Erroring,
            all_read: Rc::default(),
            cdc_committed: Rc::default(),
            cdc_read_end: Rc::default(),
        }
    }
    /// Every read resolves after `d`, so the Loading state is observable.
    #[allow(dead_code)]
    pub fn delayed(d: std::time::Duration) -> Self {
        Self {
            behavior: MockBehavior::Delayed(d),
            all_read: Rc::default(),
            cdc_committed: Rc::default(),
            cdc_read_end: Rc::default(),
        }
    }

    /// Test-only: a clone that shares this instance's CDC cursor cells but
    /// answers reads with a different `MockBehavior`. Lets a test simulate a
    /// single session whose connection recovers after an error (or stops
    /// being empty), without losing whatever the cursor already recorded —
    /// which is exactly the scenario the commit-after-a-failed-read
    /// regression needs to observe.
    #[cfg(test)]
    pub(crate) fn with_shared_cursor(&self, behavior: MockBehavior) -> Self {
        Self {
            behavior,
            all_read: self.all_read.clone(),
            cdc_committed: self.cdc_committed.clone(),
            cdc_read_end: self.cdc_read_end.clone(),
        }
    }
}

#[async_trait(?Send)]
impl ConnectionService for MockConnectionService {
    async fn list_connections(&self) -> Result<Vec<SavedConnection>, StudioError> {
        apply(self.behavior, mock::connections).await
    }

    async fn notifications(&self) -> Result<Vec<Notification>, StudioError> {
        let all_read = self.all_read.get();
        apply(self.behavior, move || {
            let mut notifs = mock::notifications();
            if all_read {
                for n in &mut notifs {
                    n.unread = false;
                }
            }
            notifs
        })
        .await
    }

    async fn mark_all_read(&self) -> Result<(), StudioError> {
        self.all_read.set(true);
        Ok(())
    }

    async fn connect(
        &self,
        name: &str,
        creds: &Credentials,
    ) -> Result<ActiveConnection, StudioError> {
        // The blank-username guard must run before any behaviour branch: a
        // caller with bad input gets `MissingUsername`, never a behaviour
        // error, even when the service is configured `Erroring`.
        if creds.username.trim().is_empty() {
            return Err(StudioError::MissingUsername);
        }
        let name = name.to_string();
        let session = apply_one(self.behavior, move || {
            mock::connections()
                .into_iter()
                .find(|c| c.name == name)
                .and_then(|c| c.open())
        })
        .await?;
        session.ok_or(StudioError::NotConnected)
    }

    async fn nav_badges(&self) -> Result<NavBadges, StudioError> {
        apply_one(self.behavior, mock::nav_badges).await
    }

    async fn session_info(&self) -> Result<SessionInfo, StudioError> {
        apply_one(self.behavior, mock::session_info).await
    }

    async fn databases(&self) -> Result<Vec<String>, StudioError> {
        apply(self.behavior, mock::databases).await
    }
}

#[async_trait(?Send)]
impl StreamsData for MockConnectionService {
    async fn cdc_feed(&self) -> Result<Vec<CdcRow>, StudioError> {
        apply(self.behavior, cdc_rows_from_mock).await
    }

    async fn open_stream_session(&self, stream: &str) -> Result<StreamSession, StudioError> {
        Ok(StreamSession {
            stream: stream.to_string(),
            group: format!("studio_{stream}"),
        })
    }

    async fn cdc_batch(
        &self,
        _session: &StreamSession,
        limit: usize,
    ) -> Result<Vec<CdcRow>, StudioError> {
        let start = self.cdc_committed.get();
        let batch: Vec<CdcRow> = cdc_rows_from_mock()
            .into_iter()
            .skip(start)
            .take(limit)
            .collect();
        // Run the behaviour first: `Erroring` returns before the cursor is
        // touched (the `?`), and `Empty` delivers no rows. Only what was
        // actually delivered may move `cdc_read_end` — otherwise a commit
        // after a failed or empty read would promote a phantom offset into
        // `cdc_committed` and silently skip events the caller never saw.
        let delivered = apply(self.behavior, move || batch).await?;
        // Do NOT advance the committed offset here: re-reading without a
        // commit must return the same rows.
        self.cdc_read_end.set(start + delivered.len());
        Ok(delivered)
    }

    async fn commit_stream_offsets(&self, _session: &StreamSession) -> Result<(), StudioError> {
        self.cdc_committed.set(self.cdc_read_end.get());
        Ok(())
    }

    async fn close_stream_session(&self, _session: &StreamSession) -> Result<(), StudioError> {
        self.cdc_committed.set(0);
        self.cdc_read_end.set(0);
        Ok(())
    }

    async fn materialized_views(&self) -> Result<Vec<MaterializedView>, StudioError> {
        apply(self.behavior, mock::materialized_views).await
    }

    async fn topics(&self) -> Result<Vec<Topic>, StudioError> {
        apply(self.behavior, mock::topics).await
    }

    async fn scheduled_jobs(&self) -> Result<Vec<ScheduledJob>, StudioError> {
        apply(self.behavior, mock::scheduled_jobs).await
    }

    async fn notify_channels(&self) -> Result<Vec<NotifyChannel>, StudioError> {
        apply(self.behavior, mock::notify_channel_rows).await
    }

    async fn notify_messages(&self) -> Result<Vec<NotifyMessage>, StudioError> {
        apply(self.behavior, mock::notify_message_rows).await
    }
}

#[async_trait(?Send)]
impl ExplorerData for MockConnectionService {
    async fn collection_groups(&self) -> Result<Vec<CollectionGroup>, StudioError> {
        apply(self.behavior, mock::collection_groups).await
    }

    async fn records(&self, collection: &str) -> Result<Vec<RecordRow>, StudioError> {
        let c = collection.to_string();
        apply(self.behavior, move || mock::records(&c)).await
    }

    async fn record_detail(&self, collection: &str, id: &str) -> Result<RecordDetail, StudioError> {
        let c = collection.to_string();
        let i = id.to_string();
        apply_one(self.behavior, move || mock::record_detail(&c, &i)).await
    }
}

#[async_trait(?Send)]
impl AdminData for MockConnectionService {
    async fn cluster_nodes(&self) -> Result<Vec<ClusterNode>, StudioError> {
        apply(self.behavior, mock::cluster_nodes).await
    }

    async fn raft_groups(&self) -> Result<Vec<RaftGroup>, StudioError> {
        apply(self.behavior, mock::raft_groups).await
    }

    async fn shard_ranges(&self) -> Result<Vec<ShardRange>, StudioError> {
        apply(self.behavior, mock::shard_ranges).await
    }

    async fn users(&self) -> Result<Vec<UserRow>, StudioError> {
        apply(self.behavior, mock::users).await
    }

    async fn rls_policies(&self) -> Result<Vec<RlsPolicy>, StudioError> {
        apply(self.behavior, mock::rls_policies).await
    }

    async fn audit_entries(&self) -> Result<Vec<AuditEntry>, StudioError> {
        apply(self.behavior, mock::audit_entries).await
    }
}

#[async_trait(?Send)]
impl WorkbenchData for MockConnectionService {
    async fn run_query(&self, sql: &str) -> Result<ResultSet, StudioError> {
        let s = sql.to_string();
        let s2 = s.clone();
        apply_one_or_empty(
            self.behavior,
            move || mock::result_set(&s),
            move || mock::empty_result_set(&s2),
        )
        .await
    }

    async fn explain(&self, sql: &str) -> Result<QueryPlan, StudioError> {
        let s = sql.to_string();
        apply_one(self.behavior, move || mock::query_plan(&s)).await
    }

    async fn schema_tree(&self) -> Result<Vec<SchemaNode>, StudioError> {
        apply(self.behavior, mock::schema_tree).await
    }
}

#[async_trait(?Send)]
impl ViewersData for MockConnectionService {
    async fn sub_graph(&self, collection: &str) -> Result<SubGraph, StudioError> {
        let c = collection.to_string();
        apply_one_or_empty(
            self.behavior,
            move || mock::sub_graph(&c),
            mock::empty_sub_graph,
        )
        .await
    }

    async fn vector_points(&self, collection: &str) -> Result<Vec<VectorPoint>, StudioError> {
        let c = collection.to_string();
        apply(self.behavior, move || mock::vector_points(&c)).await
    }

    async fn series(&self, metric: &str) -> Result<Vec<SeriesPoint>, StudioError> {
        let m = metric.to_string();
        apply(self.behavior, move || mock::series(&m)).await
    }

    async fn spatial_features(&self, collection: &str) -> Result<Vec<SpatialFeature>, StudioError> {
        let c = collection.to_string();
        apply(self.behavior, move || mock::spatial_features(&c)).await
    }

    async fn fts_hits(&self, collection: &str, query: &str) -> Result<Vec<FtsHit>, StudioError> {
        let c = collection.to_string();
        let q = query.to_string();
        apply(self.behavior, move || mock::fts_hits(&c, &q)).await
    }

    async fn sync_peers(&self) -> Result<Vec<SyncPeer>, StudioError> {
        apply(self.behavior, mock::sync_peers).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::async_state::AsyncState;
    use crate::services::streams_data::StreamsData;

    #[tokio::test]
    async fn mark_all_read_persists_across_reads() {
        let svc = MockConnectionService::ready();
        let before = svc.notifications().await.expect("mock infallible");
        assert!(before.iter().any(|n| n.unread), "fixture has unread items");
        svc.mark_all_read().await.expect("mock write infallible");
        let after = svc.notifications().await.expect("mock infallible");
        assert!(
            after.iter().all(|n| !n.unread),
            "all read after mark_all_read"
        );
    }

    #[tokio::test]
    async fn mock_notifications_returns_data() {
        let svc = MockConnectionService::ready();
        let notifs = svc
            .notifications()
            .await
            .expect("mock notifications are infallible");
        assert!(!notifs.is_empty());
    }

    #[tokio::test]
    async fn mock_list_connections_returns_data() {
        let svc = MockConnectionService::ready();
        let conns = svc
            .list_connections()
            .await
            .expect("mock list_connections is infallible");
        assert!(!conns.is_empty());
    }

    #[tokio::test]
    async fn list_connections_empty_behavior_returns_no_rows() {
        let svc = MockConnectionService::empty();
        let conns = svc.list_connections().await.expect("empty behaviour is Ok");
        assert!(conns.is_empty());
    }

    #[tokio::test]
    async fn list_connections_erroring_is_retriable_error() {
        let svc = MockConnectionService::erroring();
        let err = svc
            .list_connections()
            .await
            .expect_err("erroring must fail");
        assert!(err.is_retriable());
    }

    #[tokio::test]
    async fn notifications_empty_behavior_returns_no_rows() {
        let svc = MockConnectionService::empty();
        let notifs = svc.notifications().await.expect("empty behaviour is Ok");
        assert!(notifs.is_empty());
    }

    #[tokio::test]
    async fn notifications_erroring_is_retriable_error() {
        let svc = MockConnectionService::erroring();
        let err = svc.notifications().await.expect_err("erroring must fail");
        assert!(err.is_retriable());
    }

    #[tokio::test]
    async fn mock_connect_known_name_returns_session() {
        let svc = MockConnectionService::ready();
        let creds = Credentials {
            username: "alice".into(),
            password: None,
        };
        // `staging-cluster` is a connectable Online mock connection (data/mock.rs).
        let session = svc.connect("staging-cluster", &creds).await;
        assert!(session.is_ok());
    }

    #[tokio::test]
    async fn mock_connect_unknown_name_is_not_connected() {
        let svc = MockConnectionService::ready();
        let creds = Credentials {
            username: "alice".into(),
            password: None,
        };
        assert!(matches!(
            svc.connect("does-not-exist", &creds).await,
            Err(StudioError::NotConnected)
        ));
    }

    #[tokio::test]
    async fn connect_rejects_blank_username() {
        let svc = MockConnectionService::ready();
        let creds = Credentials {
            username: "   ".into(),
            password: None,
        };
        let out = svc.connect("local-dev", &creds).await;
        assert!(
            matches!(out, Err(StudioError::MissingUsername)),
            "blank username must be rejected, never defaulted to admin"
        );
    }

    #[tokio::test]
    async fn connect_accepts_explicit_username() {
        let svc = MockConnectionService::ready();
        let name = mock::connections()
            .first()
            .map(|c| c.name.clone())
            .expect("fixture must have at least one connection");
        let creds = Credentials {
            username: "alice".into(),
            password: None,
        };
        assert!(svc.connect(&name, &creds).await.is_ok());
    }

    #[tokio::test]
    async fn connect_erroring_is_retriable_error() {
        let svc = MockConnectionService::erroring();
        let creds = Credentials {
            username: "alice".into(),
            password: None,
        };
        let err = svc
            .connect("staging-cluster", &creds)
            .await
            .expect_err("erroring must fail");
        assert!(err.is_retriable());
    }

    #[tokio::test]
    async fn connect_delayed_still_resolves() {
        let svc = MockConnectionService::delayed(std::time::Duration::from_millis(5));
        let creds = Credentials {
            username: "alice".into(),
            password: None,
        };
        assert!(svc.connect("staging-cluster", &creds).await.is_ok());
    }

    #[tokio::test]
    async fn connect_checks_missing_username_before_consulting_behavior() {
        // A blank username must surface `MissingUsername` even when the
        // service is configured `Erroring` — the guard must not become
        // skippable by routing `connect` through the behaviour matcher.
        let svc = MockConnectionService::erroring();
        let creds = Credentials {
            username: "   ".into(),
            password: None,
        };
        let out = svc.connect("staging-cluster", &creds).await;
        assert!(
            matches!(out, Err(StudioError::MissingUsername)),
            "blank username must win over the configured Erroring behavior"
        );
    }

    #[tokio::test]
    async fn session_info_populates_the_statusbar() {
        let svc = MockConnectionService::ready();
        let s = svc.session_info().await.expect("session info");
        assert!(!s.database.is_empty());
        assert!(!s.role.is_empty());
        assert!(!s.server_version.is_empty());
        assert!(!s.timezone.is_empty());
    }

    #[tokio::test]
    async fn session_info_reports_the_fixture_values() {
        // Independently-authored expectations (not derived from the call under
        // test), so this can actually fail if the fixture drifts.
        let svc = MockConnectionService::ready();
        let s = svc.session_info().await.expect("session info");
        assert_eq!(s.database, "analytics");
        assert_eq!(s.role, "admin");
        assert!(!s.read_only, "fixture session is a read-write admin");
    }

    #[tokio::test]
    async fn session_info_empty_behavior_still_returns_the_fixture() {
        // Single-value reads have no "empty" shape, so Empty folds into Ready
        // — matching the `record_detail` / `run_query` precedent.
        let svc = MockConnectionService::empty();
        let s = svc.session_info().await.expect("empty folds into ready");
        assert_eq!(s.database, "analytics");
    }

    #[tokio::test]
    async fn session_info_erroring_is_retriable_error() {
        let svc = MockConnectionService::erroring();
        let err = svc.session_info().await.expect_err("erroring must fail");
        assert!(err.is_retriable());
    }

    #[tokio::test]
    async fn session_info_delayed_still_resolves() {
        let svc = MockConnectionService::delayed(std::time::Duration::from_millis(5));
        let s = svc.session_info().await.expect("delayed still resolves");
        assert_eq!(s.role, "admin");
    }

    #[tokio::test]
    async fn nav_badges_come_from_the_seam() {
        let svc = MockConnectionService::ready();
        let b = svc.nav_badges().await.expect("badges");
        assert!(b.query > 0 || b.streams > 0, "fixture should show badges");
    }

    #[tokio::test]
    async fn nav_badges_reports_the_fixture_counts() {
        let svc = MockConnectionService::ready();
        let b = svc.nav_badges().await.expect("badges");
        assert_eq!(b.query, 3);
        assert_eq!(b.streams, 6);
    }

    #[tokio::test]
    async fn nav_badges_empty_behavior_still_returns_the_fixture() {
        let svc = MockConnectionService::empty();
        let b = svc.nav_badges().await.expect("empty folds into ready");
        assert!(b.query > 0 || b.streams > 0);
    }

    #[tokio::test]
    async fn nav_badges_erroring_is_retriable_error() {
        let svc = MockConnectionService::erroring();
        let err = svc.nav_badges().await.expect_err("erroring must fail");
        assert!(err.is_retriable());
    }

    #[tokio::test]
    async fn nav_badges_delayed_still_resolves() {
        let svc = MockConnectionService::delayed(std::time::Duration::from_millis(5));
        let b = svc.nav_badges().await.expect("delayed still resolves");
        assert_eq!(b.query, 3);
    }

    #[tokio::test]
    async fn databases_are_unique() {
        let svc = MockConnectionService::ready();
        let mut dbs = svc.databases().await.expect("databases");
        let total = dbs.len();
        dbs.sort();
        dbs.dedup();
        assert_eq!(total, dbs.len());
    }

    #[tokio::test]
    async fn databases_fixture_has_more_than_one_entry() {
        // Pins the precondition `databases_are_unique` relies on: with a
        // single-entry fixture the dedup check above could never fire.
        let svc = MockConnectionService::ready();
        let dbs = svc.databases().await.expect("databases");
        assert!(
            dbs.len() > 1,
            "fixture must list more than one database for the dedup check to be meaningful"
        );
        assert!(dbs.contains(&"analytics".to_string()));
    }

    #[tokio::test]
    async fn databases_empty_behavior_returns_no_rows() {
        let svc = MockConnectionService::empty();
        let dbs = svc.databases().await.expect("mock databases is infallible");
        assert!(dbs.is_empty());
    }

    #[tokio::test]
    async fn databases_erroring_is_retriable_error() {
        let svc = MockConnectionService::erroring();
        let err = svc.databases().await.expect_err("erroring must fail");
        assert!(err.is_retriable());
    }

    #[tokio::test]
    async fn databases_delayed_still_returns_the_fixture() {
        let svc = MockConnectionService::delayed(std::time::Duration::from_millis(5));
        let dbs = svc.databases().await.expect("delayed still resolves");
        assert!(!dbs.is_empty());
    }

    #[tokio::test]
    async fn cdc_feed_ready_is_loaded() {
        let svc = MockConnectionService::ready();
        let state = AsyncState::from_value(Some(svc.cdc_feed().await));
        assert!(matches!(state, AsyncState::Loaded(_)));
    }

    #[tokio::test]
    async fn cdc_feed_empty_is_empty() {
        let svc = MockConnectionService::empty();
        let state = AsyncState::from_value(Some(svc.cdc_feed().await));
        assert!(state.is_empty());
    }

    #[tokio::test]
    async fn cdc_feed_erroring_is_retriable_error() {
        let svc = MockConnectionService::erroring();
        let state = AsyncState::from_value(Some(svc.cdc_feed().await));
        assert!(state.error_message().is_some());
        assert!(state.is_retriable());
    }

    // Verifies the two-step transition pattern: set Loading, await the read,
    // set Loaded from the result. This test has no Dioxus signals/guards (unit-level),
    // so it only demonstrates the transition logic; the actual no-guard-across-await
    // discipline is exercised by real streaming views and enforced by the
    // AGENTS.md convention.
    #[tokio::test]
    async fn delayed_read_transitions_loading_to_loaded_without_guard() {
        let svc = MockConnectionService::ready();
        let mut latest: AsyncState<Vec<CdcRow>> = AsyncState::Loading;
        assert!(latest.is_loading());
        // simulate the scheduler interleaving other work before the read resolves
        tokio::task::yield_now().await;
        let result = svc.cdc_feed().await;
        latest = AsyncState::from_value(Some(result));
        assert!(matches!(latest, AsyncState::Loaded(_)));
    }
}
