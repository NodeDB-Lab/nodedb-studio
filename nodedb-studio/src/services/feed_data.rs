//! Change feeds at the backend seam: the feed list, a feed's consumers and
//! events, the create/edit flow, delivery control, and Studio's own
//! observation session.
//!
//! Session reads are idempotent by design: re-reading without a commit
//! returns the same events, and only an explicit commit advances the cursor.
//! Studio opens its own consumer group (`studio_<feed>`), commits its own
//! batches, and drops the group when the tail stops. It never commits on a
//! production consumer's group, which would advance that consumer past events
//! it never processed.

use async_trait::async_trait;

use crate::models::feed_draft::{FeedDraft, FeedPreview};
use crate::models::feeds::{ChangeFeed, DeliveryAttempt, FeedConsumer, FeedEvent, FeedSession};
use crate::services::error::StudioError;

#[async_trait(?Send)]
pub trait FeedData {
    /// The change feeds of the selected database.
    async fn change_feeds(&self) -> Result<Vec<ChangeFeed>, StudioError>;

    /// The consumers of feed `feed`, with their own checkpoints.
    async fn feed_consumers(&self, feed: &str) -> Result<Vec<FeedConsumer>, StudioError>;

    /// The events the feed retained recently, newest first.
    async fn recent_events(&self, feed: &str) -> Result<Vec<FeedEvent>, StudioError>;

    /// Delivery attempts of the feed, newest first.
    async fn delivery_attempts(&self, feed: &str) -> Result<Vec<DeliveryAttempt>, StudioError>;

    /// What the server checks and reports for a draft, without creating it.
    async fn preview_feed(&self, draft: &FeedDraft) -> Result<FeedPreview, StudioError>;

    /// Create the feed and return it with its stable id.
    async fn create_feed(&self, draft: &FeedDraft) -> Result<ChangeFeed, StudioError>;

    /// Save an edit made against `revision`. `StudioError::StaleRevision`
    /// when the feed changed since: nothing is overwritten.
    async fn update_feed(
        &self,
        id: &str,
        revision: u64,
        draft: &FeedDraft,
    ) -> Result<ChangeFeed, StudioError>;

    /// Pause or resume the server's delivery. This is not Studio's tail.
    async fn set_feed_paused(&self, id: &str, paused: bool) -> Result<ChangeFeed, StudioError>;

    /// Delete the feed, its consumers, and their checkpoints.
    async fn delete_feed(&self, id: &str) -> Result<(), StudioError>;

    /// Open a Studio-owned consumer group on `feed`. Pair with
    /// `close_feed_session`.
    async fn open_feed_session(&self, feed: &str) -> Result<FeedSession, StudioError>;

    /// Up to `limit` events from the session's cursor. Idempotent.
    async fn read_feed_events(
        &self,
        session: &FeedSession,
        limit: usize,
    ) -> Result<Vec<FeedEvent>, StudioError>;

    /// Advance the session's cursor past everything read so far.
    async fn commit_feed_offsets(&self, session: &FeedSession) -> Result<(), StudioError>;

    /// Drop the Studio-owned consumer group. The feed keeps delivering.
    async fn close_feed_session(&self, session: &FeedSession) -> Result<(), StudioError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::feeds::FeedState;
    use crate::services::async_state::AsyncState;
    use crate::services::mock::MockConnectionService;
    use crate::services::mock_behavior::MockBehavior;

    fn draft() -> FeedDraft {
        let mut d = FeedDraft::blank("analytics.sales");
        d.name = "export_sales".into();
        d.endpoint = "https://warehouse.example.com/webhooks/sales".into();
        d.alias = "warehouse_sink".into();
        d
    }

    #[tokio::test]
    async fn feed_ids_are_unique() {
        let svc = MockConnectionService::ready();
        let feeds = svc.change_feeds().await.expect("feeds");
        let mut ids: Vec<_> = feeds.iter().map(|f| f.id.as_str()).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert!(n > 0);
        assert_eq!(ids.len(), n);
    }

    #[tokio::test]
    async fn empty_reaches_the_empty_state() {
        let svc = MockConnectionService::empty();
        let s = AsyncState::from_value(Some(svc.change_feeds().await));
        assert!(s.is_empty());
    }

    #[tokio::test]
    async fn erroring_reaches_the_error_state() {
        let svc = MockConnectionService::erroring();
        assert!(
            AsyncState::from_value(Some(svc.change_feeds().await))
                .error_message()
                .is_some()
        );
        assert!(svc.feed_consumers("feed_01").await.is_err());
        assert!(svc.recent_events("feed_01").await.is_err());
        assert!(svc.delivery_attempts("feed_01").await.is_err());
        assert!(svc.preview_feed(&draft()).await.is_err());
    }

    #[tokio::test]
    async fn an_unknown_feed_is_a_typed_error() {
        let svc = MockConnectionService::ready();
        assert!(matches!(
            svc.feed_consumers("feed_99").await,
            Err(StudioError::UnknownObject { .. })
        ));
        assert!(svc.set_feed_paused("feed_99", true).await.is_err());
        assert!(svc.delete_feed("feed_99").await.is_err());
    }

    #[tokio::test]
    async fn a_created_feed_appears_in_the_next_read() {
        let svc = MockConnectionService::ready();
        let f = svc.create_feed(&draft()).await.expect("creates");
        let feeds = svc.change_feeds().await.expect("feeds");
        assert!(
            feeds
                .iter()
                .any(|x| x.id == f.id && x.name == "export_sales")
        );
        assert!(svc.create_feed(&draft()).await.is_err());
    }

    #[tokio::test]
    async fn pausing_delivery_is_visible_on_the_feed() {
        let svc = MockConnectionService::ready();
        svc.set_feed_paused("feed_01", true).await.expect("pauses");
        let feeds = svc.change_feeds().await.expect("feeds");
        assert_eq!(feeds[0].state, FeedState::Paused);
    }

    #[tokio::test]
    async fn the_tail_never_changes_server_delivery() {
        let svc = MockConnectionService::ready();
        let s = svc.open_feed_session("feed_01").await.expect("session");
        svc.read_feed_events(&s, 10).await.expect("read");
        svc.commit_feed_offsets(&s).await.expect("commit");
        svc.close_feed_session(&s).await.expect("close");
        let feeds = svc.change_feeds().await.expect("feeds");
        assert_eq!(feeds[0].state, FeedState::Running);
        assert_eq!(feeds[0].checkpoint, "…d81");
    }

    #[tokio::test]
    async fn session_group_is_studio_scoped() {
        let svc = MockConnectionService::ready();
        let s = svc
            .open_feed_session("feed_01")
            .await
            .expect("session opens");
        assert_eq!(s.group, "studio_feed_01");
        assert_eq!(s.feed, "feed_01");
        assert!(svc.open_feed_session("feed_99").await.is_err());
    }

    /// Reads do not advance the cursor: two reads without a commit return the
    /// same events. A naive poll loop would therefore repeat forever.
    #[tokio::test]
    async fn reads_are_idempotent_until_committed() {
        let svc = MockConnectionService::ready();
        let s = svc.open_feed_session("feed_01").await.expect("session");
        let first = svc.read_feed_events(&s, 10).await.expect("first read");
        let second = svc.read_feed_events(&s, 10).await.expect("second read");
        assert_eq!(first, second, "an uncommitted re-read must be identical");
        assert!(!first.is_empty());
    }

    #[tokio::test]
    async fn commit_advances_past_the_batch() {
        let svc = MockConnectionService::ready();
        let s = svc.open_feed_session("feed_01").await.expect("session");
        let first = svc.read_feed_events(&s, 3).await.expect("first read");
        svc.commit_feed_offsets(&s).await.expect("commit");
        let after = svc
            .read_feed_events(&s, 3)
            .await
            .expect("read after commit");
        assert!(!after.is_empty());
        assert_ne!(first, after, "commit must advance the cursor");
    }

    #[tokio::test]
    async fn limit_is_respected() {
        let svc = MockConnectionService::ready();
        let s = svc.open_feed_session("feed_01").await.expect("session");
        assert!(svc.read_feed_events(&s, 2).await.expect("read").len() <= 2);
    }

    /// Closing drops the group by resetting the cursor, so a freshly opened
    /// session sees the same window a brand-new one would.
    #[tokio::test]
    async fn close_resets_the_cursor_for_the_next_session() {
        let svc = MockConnectionService::ready();
        let s = svc.open_feed_session("feed_01").await.expect("session");
        let first = svc.read_feed_events(&s, 3).await.expect("first read");
        svc.commit_feed_offsets(&s).await.expect("commit");
        svc.close_feed_session(&s).await.expect("close");
        let s2 = svc.open_feed_session("feed_01").await.expect("reopened");
        let again = svc
            .read_feed_events(&s2, 3)
            .await
            .expect("read after reopen");
        assert_eq!(first, again, "closing must drop the committed offset");
    }

    #[tokio::test]
    async fn commit_and_close_succeed_even_when_reads_error() {
        let svc = MockConnectionService::erroring();
        let s = svc
            .open_feed_session("feed_01")
            .await
            .expect("open is not a read");
        assert!(
            svc.read_feed_events(&s, 10).await.is_err(),
            "reads still fail"
        );
        assert!(svc.commit_feed_offsets(&s).await.is_ok());
        assert!(svc.close_feed_session(&s).await.is_ok());
    }

    #[tokio::test]
    async fn an_empty_read_delivers_no_events() {
        let svc = MockConnectionService::empty();
        let s = svc.open_feed_session("feed_01").await.expect("session");
        assert!(
            svc.read_feed_events(&s, 10)
                .await
                .expect("empty read is Ok")
                .is_empty()
        );
    }

    /// Regression: an erroring read once moved the cursor as if it had
    /// delivered every event, and a commit then skipped events the caller
    /// never saw. This swaps `Erroring` for `Ready` on the same session.
    #[tokio::test]
    async fn erroring_read_then_commit_does_not_skip_events_once_reads_recover() {
        let erroring = MockConnectionService::erroring();
        let s = erroring.open_feed_session("feed_01").await.expect("open");
        assert!(erroring.read_feed_events(&s, 10).await.is_err());
        erroring.commit_feed_offsets(&s).await.expect("commit");
        let recovered = erroring.with_shared_state(MockBehavior::Ready);
        let after = recovered
            .read_feed_events(&s, 10)
            .await
            .expect("recovered read");
        assert!(
            !after.is_empty(),
            "a commit after a failed read must not skip events"
        );
    }

    /// The same regression for an empty read.
    #[tokio::test]
    async fn empty_read_then_commit_does_not_skip_events_once_reads_recover() {
        let empty = MockConnectionService::empty();
        let s = empty.open_feed_session("feed_01").await.expect("open");
        assert!(
            empty
                .read_feed_events(&s, 10)
                .await
                .expect("empty read")
                .is_empty()
        );
        empty.commit_feed_offsets(&s).await.expect("commit");
        let recovered = empty.with_shared_state(MockBehavior::Ready);
        let after = recovered
            .read_feed_events(&s, 10)
            .await
            .expect("recovered read");
        assert!(
            !after.is_empty(),
            "a commit after an empty read must not skip events"
        );
    }
}
