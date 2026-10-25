//! Import and export jobs, kept in the service's memory.

use async_trait::async_trait;

use crate::data::mock;
use crate::models::format::epoch;
use crate::models::transfer::{
    ExportPreview, ExportRequest, ExportScope, JobKind, JobState, TransferJob,
};
use crate::services::error::StudioError;
use crate::services::mock::MockConnectionService;
use crate::services::mock_behavior::{MockBehavior, apply_one};
use crate::services::transfer_data::TransferData;

#[async_trait(?Send)]
impl TransferData for MockConnectionService {
    async fn transfer_jobs(&self) -> Result<Vec<TransferJob>, StudioError> {
        let jobs = match self.behavior {
            MockBehavior::Empty => Vec::new(),
            MockBehavior::Ready | MockBehavior::Erroring | MockBehavior::Delayed(_) => {
                self.jobs.borrow().clone()
            }
        };
        apply_one(self.behavior, move || jobs).await
    }

    async fn export_preview(&self, request: &ExportRequest) -> Result<ExportPreview, StudioError> {
        let overlay = self.records.borrow().clone();
        let r = request.clone();
        apply_one(self.behavior, move || {
            let filter = match &r.scope {
                ExportScope::All => None,
                ExportScope::Filtered(f) => Some(f),
            };
            let (rows, total) = mock::transfer_preview(&r.collection, filter, &r.fields, &overlay)
                .ok_or_else(|| StudioError::RecordMissing {
                    collection: r.collection.clone(),
                    id: String::new(),
                })?;
            Ok(ExportPreview {
                columns: r.fields.clone(),
                rows,
                total,
            })
        })
        .await?
    }

    async fn start_export(&self, request: &ExportRequest) -> Result<TransferJob, StudioError> {
        let r = request.clone();
        let next = self.jobs.borrow().len() + 1;
        let job = apply_one(self.behavior, move || {
            if r.fields.is_empty() {
                return Err(StudioError::Rejected {
                    reason: "Select at least one field to export.".to_string(),
                });
            }
            Ok(TransferJob {
                id: format!("job_{:04}", 412 + next),
                name: format!("{}.{}", r.collection, r.format.extension()),
                kind: JobKind::Export,
                collection: r.collection,
                state: JobState::Running { percent: 0 },
                started_at: mock::observed_at().max(epoch(2026, 6, 13, 13, 40, 0)),
            })
        })
        .await??;
        self.jobs.borrow_mut().insert(0, job.clone());
        Ok(job)
    }

    async fn cancel_job(&self, id: &str) -> Result<(), StudioError> {
        apply_one(self.behavior, || ()).await?;
        let mut jobs = self.jobs.borrow_mut();
        let Some(job) = jobs.iter_mut().find(|j| j.id == id) else {
            return Err(StudioError::RecordMissing {
                collection: "jobs".to_string(),
                id: id.to_string(),
            });
        };
        if !job.running() {
            return Err(StudioError::Rejected {
                reason: format!("{} already {}.", job.name, job.state.label().to_lowercase()),
            });
        }
        job.state = JobState::Cancelled;
        Ok(())
    }
}
