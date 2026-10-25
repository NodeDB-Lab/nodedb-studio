//! The in-process backend: answers every seam read from `crate::data::mock`.
//! One file per domain trait implements it, next to this one.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;

use crate::data::mock;
use crate::models::transfer::TransferJob;
use crate::services::mock_behavior::MockBehavior;
use crate::state::connections_registry::SavedConnection;

/// Hardcoded implementation used while the real client is not wired. Data
/// comes from `crate::data::mock`; writes apply to the in-memory state below.
///
/// Fields are `Rc<Cell/RefCell>` for interior mutability: a write persists
/// across later reads without blocking the single-threaded Dioxus runtime.
/// Clones share the state.
#[derive(Debug, Clone)]
pub struct MockConnectionService {
    pub(super) behavior: MockBehavior,
    /// Set once `mark_all_read` succeeds. Later feed reads show every entry read.
    pub(super) all_read: Rc<Cell<bool>>,
    /// Ids `mark_read` has marked.
    pub(super) read: Rc<RefCell<BTreeSet<String>>>,
    /// The saved-connection registry. Saves and duplicates write here.
    pub(super) saved: Rc<RefCell<Vec<SavedConnection>>>,
    /// The saved-connection name the last successful `connect` opened.
    /// Session-scoped reads (`databases`) answer for it, as a server does.
    pub(super) session: Rc<RefCell<Option<String>>>,
    /// Change feeds, with Studio's tail session cursors. Only a commit
    /// advances a cursor, mirroring the server: reads are idempotent.
    pub(super) feeds: Rc<RefCell<mock::FeedStore>>,
    /// Topics, their logs, and consumer group offsets.
    pub(super) topics: Rc<RefCell<mock::TopicStore>>,
    /// Materialized views and their refresh history.
    pub(super) views: Rc<RefCell<mock::ViewStore>>,
    /// Listening sessions and the notifications queued for them.
    pub(super) listen: Rc<RefCell<mock::ListenStore>>,
    /// Scheduled jobs and their runs.
    pub(super) schedules: Rc<RefCell<mock::JobStore>>,
    /// Record writes made this session, laid over the Explorer fixtures.
    pub(super) records: Rc<RefCell<mock::Overlay>>,
    /// Import and export jobs, newest first. Starting and cancelling write here.
    pub(super) jobs: Rc<RefCell<Vec<TransferJob>>>,
    /// The transaction, saved queries, and console history. Writes change
    /// them and every later read sees the change.
    pub(super) workbench: Rc<RefCell<mock::WorkbenchStore>>,
    /// Staged schema changes and the schema changes applied. An applied
    /// collection joins the catalog the Explorer reads.
    pub(super) designer: Rc<RefCell<mock::DesignerStore>>,
    /// Saved dashboards, the `sales` lifecycle, and backfill jobs.
    pub(super) timeseries: Rc<RefCell<mock::TimeseriesStore>>,
    /// Search executions and the rebuild jobs created this session.
    pub(super) search: Rc<RefCell<mock::SearchStore>>,
    /// Raft groups, placement, and activity, with the operations applied.
    pub(super) cluster: Rc<RefCell<mock::ClusterStore>>,
    /// Devices, subscriptions, rejected writes, and activity, with the
    /// writes applied to them.
    pub(super) sync: Rc<RefCell<mock::SyncStore>>,
    /// Principals, roles, policies, the audit trail, snapshots, config, and
    /// credentials, with the changes applied to them.
    pub(super) admin: Rc<RefCell<mock::AdminStore>>,
}

impl Default for MockConnectionService {
    fn default() -> Self {
        Self::ready()
    }
}

// Constructors are public API for demos and tests; not all are used in the app binary.
impl MockConnectionService {
    fn with(behavior: MockBehavior) -> Self {
        Self {
            behavior,
            all_read: Rc::default(),
            read: Rc::default(),
            saved: Rc::new(RefCell::new(mock::connections())),
            session: Rc::default(),
            feeds: Rc::new(RefCell::new(mock::FeedStore::seeded())),
            topics: Rc::new(RefCell::new(mock::TopicStore::seeded())),
            views: Rc::new(RefCell::new(mock::ViewStore::seeded())),
            listen: Rc::new(RefCell::new(mock::ListenStore::seeded())),
            schedules: Rc::new(RefCell::new(mock::JobStore::seeded())),
            records: Rc::default(),
            jobs: Rc::new(RefCell::new(mock::transfer_jobs())),
            workbench: Rc::new(RefCell::new(mock::WorkbenchStore::seeded())),
            designer: Rc::new(RefCell::new(mock::DesignerStore::seeded())),
            timeseries: Rc::new(RefCell::new(mock::TimeseriesStore::seeded())),
            search: Rc::default(),
            cluster: Rc::new(RefCell::new(mock::ClusterStore::seeded())),
            sync: Rc::new(RefCell::new(mock::SyncStore::seeded())),
            admin: Rc::new(RefCell::new(mock::AdminStore::seeded())),
        }
    }

    /// Rich fixtures (the default).
    pub fn ready() -> Self {
        Self::with(MockBehavior::Ready)
    }
    /// Every read returns an empty collection.
    #[allow(dead_code)]
    pub fn empty() -> Self {
        Self::with(MockBehavior::Empty)
    }
    /// Every read fails with a retriable server error.
    #[allow(dead_code)]
    pub fn erroring() -> Self {
        Self::with(MockBehavior::Erroring)
    }
    /// Every read resolves after `d`, so the Loading state is observable.
    #[allow(dead_code)]
    pub fn delayed(d: std::time::Duration) -> Self {
        Self::with(MockBehavior::Delayed(d))
    }

    /// Test-only: the same session with a principal that lacks the
    /// subscription privilege.
    #[cfg(test)]
    pub(crate) fn without_sync_permission(self) -> Self {
        Self {
            sync: Rc::new(RefCell::new(mock::SyncStore::without_permission())),
            ..self
        }
    }

    /// Test-only: the same session with a principal that holds none of the
    /// privileges an administration change needs.
    #[cfg(test)]
    pub(crate) fn without_admin_permission(self) -> Self {
        Self {
            admin: Rc::new(RefCell::new(mock::AdminStore::without_permission())),
            ..self
        }
    }

    /// Test-only: a clone that shares this instance's state cells but
    /// answers reads with a different `MockBehavior`. Lets a test simulate a
    /// single session whose connection recovers after an error (or stops
    /// being empty), without losing whatever the cursor already recorded.
    /// That is the scenario the commit-after-a-failed-read regression needs.
    #[cfg(test)]
    pub(crate) fn with_shared_state(&self, behavior: MockBehavior) -> Self {
        Self {
            behavior,
            ..self.clone()
        }
    }
}
