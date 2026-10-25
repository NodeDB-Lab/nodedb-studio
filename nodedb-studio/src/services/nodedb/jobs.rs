//! Scheduled jobs against a real server. Not wired yet.

use async_trait::async_trait;

use crate::models::jobs::{JobForm, JobList, JobRun, LogLine, ScheduledJob};
use crate::services::error::StudioError;
use crate::services::job_data::JobData;
use crate::services::nodedb::NodeDbConnectionService;

#[async_trait(?Send)]
impl JobData for NodeDbConnectionService {
    async fn scheduled_jobs(&self) -> Result<JobList, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn job_runs(&self, _job: &str) -> Result<Vec<JobRun>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn job_log(&self, _run: &str) -> Result<Vec<LogLine>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn create_job(&self, _form: &JobForm) -> Result<ScheduledJob, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn save_job(
        &self,
        _id: &str,
        _revision: u64,
        _form: &JobForm,
    ) -> Result<ScheduledJob, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn set_job_enabled(
        &self,
        _id: &str,
        _enabled: bool,
    ) -> Result<ScheduledJob, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn run_job_now(&self, _id: &str) -> Result<JobRun, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn cancel_job_run(&self, _run: &str) -> Result<JobRun, StudioError> {
        Err(StudioError::NotConnected)
    }
}
