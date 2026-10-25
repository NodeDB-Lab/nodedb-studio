//! Change feeds against a real server. Not wired yet.

use async_trait::async_trait;

use crate::models::feed_draft::{FeedDraft, FeedPreview};
use crate::models::feeds::{ChangeFeed, DeliveryAttempt, FeedConsumer, FeedEvent, FeedSession};
use crate::services::error::StudioError;
use crate::services::feed_data::FeedData;
use crate::services::nodedb::NodeDbConnectionService;

#[async_trait(?Send)]
impl FeedData for NodeDbConnectionService {
    async fn change_feeds(&self) -> Result<Vec<ChangeFeed>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn feed_consumers(&self, _feed: &str) -> Result<Vec<FeedConsumer>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn recent_events(&self, _feed: &str) -> Result<Vec<FeedEvent>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn delivery_attempts(&self, _feed: &str) -> Result<Vec<DeliveryAttempt>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn preview_feed(&self, _draft: &FeedDraft) -> Result<FeedPreview, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn create_feed(&self, _draft: &FeedDraft) -> Result<ChangeFeed, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn update_feed(
        &self,
        _id: &str,
        _revision: u64,
        _draft: &FeedDraft,
    ) -> Result<ChangeFeed, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn set_feed_paused(&self, _id: &str, _paused: bool) -> Result<ChangeFeed, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn delete_feed(&self, _id: &str) -> Result<(), StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn open_feed_session(&self, _feed: &str) -> Result<FeedSession, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn read_feed_events(
        &self,
        _session: &FeedSession,
        _limit: usize,
    ) -> Result<Vec<FeedEvent>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn commit_feed_offsets(&self, _session: &FeedSession) -> Result<(), StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn close_feed_session(&self, _session: &FeedSession) -> Result<(), StudioError> {
        Err(StudioError::NotConnected)
    }
}
