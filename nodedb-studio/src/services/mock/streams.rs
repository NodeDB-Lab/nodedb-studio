//! Change feeds, topics, materialized views, notifications, and jobs.

use async_trait::async_trait;

use crate::data::mock;
use crate::models::cdc::CdcRow;
use crate::models::streams::{
    MaterializedView, NotifyChannel, NotifyMessage, ScheduledJob, StreamSession, Topic,
};
use crate::services::error::StudioError;
use crate::services::mock::MockConnectionService;
use crate::services::mock_behavior::apply;
use crate::services::streams_data::{StreamsData, cdc_rows_from_mock};

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
