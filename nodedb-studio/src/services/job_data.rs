//! Scheduled jobs at the backend seam: the job list, run history and logs,
//! the create and edit flow, and the run controls.

use async_trait::async_trait;

use crate::models::jobs::{JobForm, JobList, JobRun, LogLine, ScheduledJob};
use crate::services::error::StudioError;

#[async_trait(?Send)]
pub trait JobData {
    /// The jobs of the selected database and the server time they were read.
    async fn scheduled_jobs(&self) -> Result<JobList, StudioError>;

    /// The runs of a job, newest first.
    async fn job_runs(&self, job: &str) -> Result<Vec<JobRun>, StudioError>;

    /// The log of one run.
    async fn job_log(&self, run: &str) -> Result<Vec<LogLine>, StudioError>;

    /// Create a job and return it with its stable id.
    async fn create_job(&self, form: &JobForm) -> Result<ScheduledJob, StudioError>;

    /// Save an edit made against `revision`. `StudioError::StaleRevision`
    /// when the job changed since: nothing is overwritten.
    async fn save_job(
        &self,
        id: &str,
        revision: u64,
        form: &JobForm,
    ) -> Result<ScheduledJob, StudioError>;

    /// Enable or disable the schedule. A disabled job keeps its history.
    async fn set_job_enabled(&self, id: &str, enabled: bool) -> Result<ScheduledJob, StudioError>;

    /// Start a run now, outside the schedule.
    async fn run_job_now(&self, id: &str) -> Result<JobRun, StudioError>;

    /// Cancel one running run. The schedule is unchanged.
    async fn cancel_job_run(&self, run: &str) -> Result<JobRun, StudioError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::jobs::RunState;
    use crate::services::async_state::AsyncState;
    use crate::services::mock::MockConnectionService;

    #[tokio::test]
    async fn job_ids_are_unique() {
        let svc = MockConnectionService::ready();
        let jobs = svc.scheduled_jobs().await.expect("jobs").jobs;
        let mut ids: Vec<_> = jobs.iter().map(|j| j.id.as_str()).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert!(n > 0);
        assert_eq!(ids.len(), n);
    }

    #[tokio::test]
    async fn empty_reaches_the_empty_state() {
        let svc = MockConnectionService::empty();
        assert!(AsyncState::from_value(Some(svc.scheduled_jobs().await)).is_empty());
    }

    #[tokio::test]
    async fn erroring_reaches_the_error_state() {
        let svc = MockConnectionService::erroring();
        assert!(
            AsyncState::from_value(Some(svc.scheduled_jobs().await))
                .error_message()
                .is_some()
        );
        assert!(svc.job_runs("job_01").await.is_err());
        assert!(svc.job_log("job_01_run5").await.is_err());
    }

    #[tokio::test]
    async fn the_fixture_has_a_failed_and_a_successful_run() {
        let svc = MockConnectionService::ready();
        let runs = svc.job_runs("job_01").await.expect("runs");
        assert!(runs.iter().any(|r| r.state == RunState::Success));
        assert!(runs.iter().any(|r| r.state == RunState::Failed));
    }

    #[tokio::test]
    async fn run_now_then_cancel_targets_the_run_not_the_schedule() {
        let svc = MockConnectionService::ready();
        let run = svc.run_job_now("job_01").await.expect("starts");
        let cancelled = svc.cancel_job_run(&run.id).await.expect("cancels");
        assert_eq!(cancelled.state, RunState::Cancelled);
        let jobs = svc.scheduled_jobs().await.expect("jobs").jobs;
        assert!(jobs[0].enabled);
    }

    #[tokio::test]
    async fn disabling_a_job_is_visible_in_the_next_read() {
        let svc = MockConnectionService::ready();
        svc.set_job_enabled("job_01", false)
            .await
            .expect("disables");
        assert!(!svc.scheduled_jobs().await.expect("jobs").jobs[0].enabled);
    }

    #[tokio::test]
    async fn a_saved_edit_bumps_the_revision_and_a_stale_one_is_refused() {
        let svc = MockConnectionService::ready();
        let job = svc.scheduled_jobs().await.expect("jobs").jobs.remove(0);
        let mut form = JobForm::of(&job);
        form.retries = "5".into();
        let saved = svc
            .save_job(&job.id, job.revision, &form)
            .await
            .expect("saves");
        assert_eq!(saved.revision, job.revision + 1);
        assert!(matches!(
            svc.save_job(&job.id, job.revision, &form).await,
            Err(StudioError::StaleRevision { .. })
        ));
    }
}
