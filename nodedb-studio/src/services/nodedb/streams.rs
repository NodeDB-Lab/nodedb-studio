//! Change feeds, topics, materialized views, notifications, and jobs.

use async_trait::async_trait;

use crate::models::cdc::CdcRow;
use crate::models::streams::{
    MaterializedView, NotifyChannel, NotifyMessage, ScheduledJob, StreamSession, Topic,
};
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;
use crate::services::streams_data::StreamsData;

#[async_trait(?Send)]
impl StreamsData for NodeDbConnectionService {
    async fn cdc_feed(&self) -> Result<Vec<CdcRow>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn open_stream_session(&self, _stream: &str) -> Result<StreamSession, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn cdc_batch(
        &self,
        _session: &StreamSession,
        _limit: usize,
    ) -> Result<Vec<CdcRow>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn commit_stream_offsets(&self, _session: &StreamSession) -> Result<(), StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn close_stream_session(&self, _session: &StreamSession) -> Result<(), StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn materialized_views(&self) -> Result<Vec<MaterializedView>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn topics(&self) -> Result<Vec<Topic>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn scheduled_jobs(&self) -> Result<Vec<ScheduledJob>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn notify_channels(&self) -> Result<Vec<NotifyChannel>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn notify_messages(&self) -> Result<Vec<NotifyMessage>, StudioError> {
        Err(StudioError::NotConnected)
    }
}
