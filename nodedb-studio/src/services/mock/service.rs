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
    /// Offset the next `cdc_batch` reads from. Only `commit_stream_offsets`
    /// advances it, mirroring the server: reads are idempotent.
    pub(super) cdc_committed: Rc<Cell<usize>>,
    /// How far the most recent `cdc_batch` read. Commit promotes this into
    /// `cdc_committed`.
    pub(super) cdc_read_end: Rc<Cell<usize>>,
    /// Record writes made this session, laid over the Explorer fixtures.
    pub(super) records: Rc<RefCell<mock::Overlay>>,
    /// Import and export jobs, newest first. Starting and cancelling write here.
    pub(super) jobs: Rc<RefCell<Vec<TransferJob>>>,
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
            cdc_committed: Rc::default(),
            cdc_read_end: Rc::default(),
            records: Rc::default(),
            jobs: Rc::new(RefCell::new(mock::transfer_jobs())),
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

    /// Test-only: a clone that shares this instance's state cells but
    /// answers reads with a different `MockBehavior`. Lets a test simulate a
    /// single session whose connection recovers after an error (or stops
    /// being empty), without losing whatever the cursor already recorded —
    /// which is exactly the scenario the commit-after-a-failed-read
    /// regression needs to observe.
    #[cfg(test)]
    pub(crate) fn with_shared_state(&self, behavior: MockBehavior) -> Self {
        Self {
            behavior,
            ..self.clone()
        }
    }
}
