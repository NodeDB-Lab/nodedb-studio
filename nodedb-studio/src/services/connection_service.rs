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
use crate::models::streams::{
    MaterializedView, NotifyChannel, NotifyMessage, ScheduledJob, StreamSession, Topic,
};
use crate::services::admin_data::AdminData;
use crate::services::error::StudioError;
use crate::services::explorer_data::ExplorerData;
use crate::services::mock_behavior::{MockBehavior, apply};
use crate::services::streams_data::{StreamsData, cdc_rows_from_mock};
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
        Ok(mock::connections())
    }

    async fn notifications(&self) -> Result<Vec<Notification>, StudioError> {
        let mut notifs = mock::notifications();
        if self.all_read.get() {
            for n in &mut notifs {
                n.unread = false;
            }
        }
        Ok(notifs)
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
        if creds.username.trim().is_empty() {
            return Err(StudioError::MissingUsername);
        }
        mock::connections()
            .into_iter()
            .find(|c| c.name == name)
            .and_then(|c| c.open())
            .ok_or(StudioError::NotConnected)
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
        match self.behavior {
            MockBehavior::Erroring => Err(StudioError::from(
                nodedb_client::NodeDbError::node_unreachable("mock"),
            )),
            MockBehavior::Ready | MockBehavior::Empty => Ok(mock::record_detail(collection, id)),
            MockBehavior::Delayed(d) => {
                tokio::time::sleep(d).await;
                Ok(mock::record_detail(collection, id))
            }
        }
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
    // discipline is exercised by the real streaming view (later task) and enforced
    // by the AGENTS.md convention.
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
