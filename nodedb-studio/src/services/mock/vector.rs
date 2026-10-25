//! Vector Space reads. Search and index status answer from fixtures. A
//! projection is computed: the sample is drawn and reduced to two dimensions
//! on a worker thread, so the window stays responsive while it runs.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{TryRecvError, channel};
use std::time::Duration;

use async_trait::async_trait;

use crate::data::mock;
use crate::models::vector::{NeighborRequest, NeighborResult, VectorRecord, VectorSetup};
use crate::models::vector_index::IndexStatus;
use crate::models::vector_projection::{Projection, ProjectionRequest};
use crate::services::error::StudioError;
use crate::services::mock::MockConnectionService;
use crate::services::mock_behavior::{MockBehavior, apply_one_result};
use crate::services::vector_data::VectorData;

/// Tells the worker to stop when the awaiting future is dropped.
struct StopOnDrop(Arc<AtomicBool>);

impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// Run `job` on its own thread and wait for it without blocking the runtime.
/// Dropping the future sets the flag the job polls, so a canceled projection
/// stops and its output is never read.
async fn on_worker<T: Send + 'static>(
    job: impl FnOnce(&dyn Fn() -> bool) -> T + Send + 'static,
) -> Result<T, StudioError> {
    let stop = Arc::new(AtomicBool::new(false));
    let guard = StopOnDrop(stop.clone());
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let canceled = || stop.load(Ordering::Relaxed);
        // The receiver is gone when the future was dropped.
        let _ = tx.send(job(&canceled));
    });
    let out = loop {
        match rx.try_recv() {
            Ok(v) => break Ok(v),
            Err(TryRecvError::Empty) => tokio::time::sleep(Duration::from_millis(10)).await,
            Err(TryRecvError::Disconnected) => {
                break Err(StudioError::Rejected {
                    reason: "The projection worker stopped without a result.".to_string(),
                });
            }
        }
    };
    drop(guard);
    out
}

#[async_trait(?Send)]
impl VectorData for MockConnectionService {
    async fn vector_setup(&self) -> Result<VectorSetup, StudioError> {
        let mut setup = mock::vector_setup();
        if self.behavior == MockBehavior::Empty {
            setup.targets.clear();
        }
        apply_one_result(self.behavior, Ok(setup)).await
    }

    async fn vector_record(
        &self,
        collection: &str,
        field: &str,
        id: &str,
    ) -> Result<VectorRecord, StudioError> {
        apply_one_result(self.behavior, mock::vector_record(collection, field, id)).await
    }

    async fn vector_search(
        &self,
        request: &NeighborRequest,
    ) -> Result<NeighborResult, StudioError> {
        let mut read = mock::vector_search(request);
        if self.behavior == MockBehavior::Empty
            && let Ok(r) = &mut read
        {
            r.neighbors.clear();
        }
        apply_one_result(self.behavior, read).await
    }

    async fn vector_project(&self, request: &ProjectionRequest) -> Result<Projection, StudioError> {
        match self.behavior {
            MockBehavior::Ready | MockBehavior::Empty => {}
            MockBehavior::Erroring => {
                return apply_one_result::<Projection>(
                    self.behavior,
                    Err(StudioError::NotConnected),
                )
                .await;
            }
            MockBehavior::Delayed(d) => tokio::time::sleep(d).await,
        }
        let request = request.clone();
        on_worker(move |canceled| mock::vector_project(&request, canceled)).await?
    }

    async fn vector_index_status(
        &self,
        collection: &str,
        field: &str,
    ) -> Result<IndexStatus, StudioError> {
        apply_one_result(self.behavior, mock::vector_index_status(collection, field)).await
    }
}
