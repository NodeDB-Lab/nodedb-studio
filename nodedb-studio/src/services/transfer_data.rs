//! Import and export jobs at the backend seam.
//!
//! A job runs on the server. Studio starts it, lists it, and cancels it.

use async_trait::async_trait;

use crate::models::transfer::{ExportPreview, ExportRequest, TransferJob};
use crate::services::error::StudioError;

#[async_trait(?Send)]
pub trait TransferData {
    /// Every job of the active database, newest first.
    async fn transfer_jobs(&self) -> Result<Vec<TransferJob>, StudioError>;

    /// The first rows `request` would write, and how many it would write.
    async fn export_preview(&self, request: &ExportRequest) -> Result<ExportPreview, StudioError>;

    /// Start an export. The job is running when this returns.
    async fn start_export(&self, request: &ExportRequest) -> Result<TransferJob, StudioError>;

    /// Cancel a running job. `StudioError::Rejected` when it already ended.
    async fn cancel_job(&self, id: &str) -> Result<(), StudioError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::records::Filter;
    use crate::models::transfer::{Delimiter, ExportFormat, ExportScope, JobState};
    use crate::services::mock::MockConnectionService;

    fn request(scope: ExportScope) -> ExportRequest {
        ExportRequest {
            collection: "users".into(),
            format: ExportFormat::Csv,
            scope,
            fields: ["id", "name", "email", "status"].map(String::from).to_vec(),
            header: true,
            delimiter: Delimiter::Comma,
        }
    }

    fn active() -> ExportScope {
        ExportScope::Filtered(Filter {
            field: "status".into(),
            value: "active".into(),
        })
    }

    #[tokio::test]
    async fn job_ids_are_unique_and_a_running_job_is_listed() {
        let svc = MockConnectionService::ready();
        let jobs = svc.transfer_jobs().await.expect("jobs");
        let mut ids: Vec<&str> = jobs.iter().map(|j| j.id.as_str()).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(n, ids.len());
        assert!(jobs.iter().any(|j| j.running()));
    }

    #[tokio::test]
    async fn the_preview_counts_the_filtered_rows() {
        let svc = MockConnectionService::ready();
        let p = svc
            .export_preview(&request(active()))
            .await
            .expect("preview");
        assert_eq!(p.total, 842);
        assert_eq!(p.rows.len(), 3);
        assert_eq!(p.rows[0].len(), 4);
        let all = svc
            .export_preview(&request(ExportScope::All))
            .await
            .expect("preview");
        assert_eq!(all.total, 1284);
    }

    #[tokio::test]
    async fn starting_an_export_adds_a_running_job_and_cancel_ends_it() {
        let svc = MockConnectionService::ready();
        let job = svc.start_export(&request(active())).await.expect("start");
        assert_eq!(job.name, "users.csv");
        assert!(job.running());
        assert_eq!(svc.transfer_jobs().await.expect("jobs")[0].id, job.id);
        svc.cancel_job(&job.id).await.expect("cancel");
        let after = svc.transfer_jobs().await.expect("jobs");
        assert_eq!(after[0].state, JobState::Cancelled);
        assert!(matches!(
            svc.cancel_job(&job.id).await,
            Err(StudioError::Rejected { .. })
        ));
    }

    #[tokio::test]
    async fn an_export_with_no_fields_is_rejected() {
        let svc = MockConnectionService::ready();
        let mut r = request(ExportScope::All);
        r.fields.clear();
        assert!(matches!(
            svc.start_export(&r).await,
            Err(StudioError::Rejected { .. })
        ));
    }

    #[tokio::test]
    async fn empty_and_erroring_behaviours_are_reachable() {
        let empty = MockConnectionService::empty();
        assert!(empty.transfer_jobs().await.expect("jobs").is_empty());
        assert!(
            MockConnectionService::erroring()
                .transfer_jobs()
                .await
                .is_err()
        );
    }
}
