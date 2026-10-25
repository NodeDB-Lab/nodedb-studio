//! Scheduled jobs: the list, runs, logs, and the run controls.

use async_trait::async_trait;

use crate::models::jobs::{JobForm, JobList, JobRun, LogLine, ScheduledJob};
use crate::services::error::StudioError;
use crate::services::job_data::JobData;
use crate::services::mock::MockConnectionService;
use crate::services::mock_behavior::{MockBehavior, apply_one_result, apply_result, apply_write};

#[async_trait(?Send)]
impl JobData for MockConnectionService {
    async fn scheduled_jobs(&self) -> Result<JobList, StudioError> {
        let mut list = self.schedules.borrow().list();
        // `Empty` keeps the server clock but lists no jobs.
        if self.behavior == MockBehavior::Empty {
            list.jobs.clear();
        }
        apply_one_result(self.behavior, Ok(list)).await
    }

    async fn job_runs(&self, job: &str) -> Result<Vec<JobRun>, StudioError> {
        let runs = self.schedules.borrow_mut().runs(job);
        apply_result(self.behavior, runs).await
    }

    async fn job_log(&self, run: &str) -> Result<Vec<LogLine>, StudioError> {
        let read = self.schedules.borrow().log(run);
        apply_result(self.behavior, read).await
    }

    async fn create_job(&self, form: &JobForm) -> Result<ScheduledJob, StudioError> {
        apply_write(self.behavior, || self.schedules.borrow_mut().create(form)).await
    }

    async fn save_job(
        &self,
        id: &str,
        revision: u64,
        form: &JobForm,
    ) -> Result<ScheduledJob, StudioError> {
        apply_write(self.behavior, || {
            self.schedules.borrow_mut().save(id, revision, form)
        })
        .await
    }

    async fn set_job_enabled(&self, id: &str, enabled: bool) -> Result<ScheduledJob, StudioError> {
        apply_write(self.behavior, || {
            self.schedules.borrow_mut().set_enabled(id, enabled)
        })
        .await
    }

    async fn run_job_now(&self, id: &str) -> Result<JobRun, StudioError> {
        apply_write(self.behavior, || self.schedules.borrow_mut().run_now(id)).await
    }

    async fn cancel_job_run(&self, run: &str) -> Result<JobRun, StudioError> {
        apply_write(self.behavior, || self.schedules.borrow_mut().cancel(run)).await
    }
}
