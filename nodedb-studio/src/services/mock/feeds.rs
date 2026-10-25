//! Change feeds: the list, consumers, events, writes, and Studio's session.

use async_trait::async_trait;

use crate::data::mock;
use crate::models::feed_draft::{FeedDraft, FeedPreview};
use crate::models::feeds::{ChangeFeed, DeliveryAttempt, FeedConsumer, FeedEvent, FeedSession};
use crate::services::error::StudioError;
use crate::services::feed_data::FeedData;
use crate::services::mock::MockConnectionService;
use crate::services::mock_behavior::{apply, apply_one, apply_result, apply_write};

#[async_trait(?Send)]
impl FeedData for MockConnectionService {
    async fn change_feeds(&self) -> Result<Vec<ChangeFeed>, StudioError> {
        apply(self.behavior, || self.feeds.borrow().list()).await
    }

    async fn feed_consumers(&self, feed: &str) -> Result<Vec<FeedConsumer>, StudioError> {
        let read = self.feeds.borrow().consumers(feed);
        apply_result(self.behavior, read).await
    }

    async fn recent_events(&self, feed: &str) -> Result<Vec<FeedEvent>, StudioError> {
        let read = self.feeds.borrow().recent(feed);
        apply_result(self.behavior, read).await
    }

    async fn delivery_attempts(&self, feed: &str) -> Result<Vec<DeliveryAttempt>, StudioError> {
        let read = self.feeds.borrow().attempts(feed);
        apply_result(self.behavior, read).await
    }

    async fn preview_feed(&self, draft: &FeedDraft) -> Result<FeedPreview, StudioError> {
        apply_one(self.behavior, || mock::preview_feed(draft)).await
    }

    async fn create_feed(&self, draft: &FeedDraft) -> Result<ChangeFeed, StudioError> {
        apply_write(self.behavior, || self.feeds.borrow_mut().create(draft)).await
    }

    async fn update_feed(
        &self,
        id: &str,
        revision: u64,
        draft: &FeedDraft,
    ) -> Result<ChangeFeed, StudioError> {
        apply_write(self.behavior, || {
            self.feeds.borrow_mut().update(id, revision, draft)
        })
        .await
    }

    async fn set_feed_paused(&self, id: &str, paused: bool) -> Result<ChangeFeed, StudioError> {
        apply_write(self.behavior, || {
            self.feeds.borrow_mut().set_paused(id, paused)
        })
        .await
    }

    async fn delete_feed(&self, id: &str) -> Result<(), StudioError> {
        apply_write(self.behavior, || self.feeds.borrow_mut().delete(id)).await
    }

    async fn open_feed_session(&self, feed: &str) -> Result<FeedSession, StudioError> {
        // Opening a session is not a read: it works while reads fail.
        self.feeds.borrow().consumers(feed)?;
        Ok(FeedSession {
            feed: feed.to_string(),
            group: format!("studio_{feed}"),
        })
    }

    async fn read_feed_events(
        &self,
        session: &FeedSession,
        limit: usize,
    ) -> Result<Vec<FeedEvent>, StudioError> {
        let batch = self.feeds.borrow().peek(&session.feed, limit)?;
        // Run the behaviour first: `Erroring` returns before the cursor is
        // touched, and `Empty` delivers nothing. Only what was delivered may
        // move the read end. Otherwise a commit after a failed or empty read
        // would promote a phantom offset and skip events nobody saw.
        let delivered = apply(self.behavior, move || batch).await?;
        // The committed offset does not move here: a re-read without a
        // commit returns the same events.
        self.feeds
            .borrow_mut()
            .mark_read(&session.feed, delivered.len());
        Ok(delivered)
    }

    async fn commit_feed_offsets(&self, session: &FeedSession) -> Result<(), StudioError> {
        self.feeds.borrow_mut().commit(&session.feed);
        Ok(())
    }

    async fn close_feed_session(&self, session: &FeedSession) -> Result<(), StudioError> {
        self.feeds.borrow_mut().reset(&session.feed);
        Ok(())
    }
}
