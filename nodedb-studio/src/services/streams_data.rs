//! Streams-tier reads at the backend seam. One method per Streams screen's data,
//! plus the CDC consumer-group lifecycle (open/read/commit/close).
//!
//! CDC reads are idempotent by design: re-reading without committing returns
//! the same rows, and only an explicit commit advances the cursor. A poll loop
//! that never commits re-reads the same window forever; one that commits on a
//! *shared* consumer group advances a production consumer past events it never
//! processed. Studio therefore always opens its own group (`studio_<stream>`),
//! commits its own batches, and drops the group on disconnect.

use async_trait::async_trait;

use crate::models::cdc::{CdcOp, CdcRow};
use crate::models::streams::{
    MaterializedView, NotifyChannel, NotifyMessage, ScheduledJob, StreamSession, Topic,
};
use crate::services::error::StudioError;

#[async_trait(?Send)]
pub trait StreamsData {
    /// The CDC change feed, newest first.
    async fn cdc_feed(&self) -> Result<Vec<CdcRow>, StudioError>;

    /// Create a Studio-owned consumer group on `stream` and return the session.
    /// Callers must pair this with `close_stream_session`.
    #[allow(dead_code)] // SEAM-UNWIRED
    async fn open_stream_session(&self, stream: &str) -> Result<StreamSession, StudioError>;

    /// Read up to `limit` events from the session's current cursor. Idempotent:
    /// re-reading without committing returns the same events.
    #[allow(dead_code)] // SEAM-UNWIRED
    async fn cdc_batch(
        &self,
        session: &StreamSession,
        limit: usize,
    ) -> Result<Vec<CdcRow>, StudioError>;

    /// Advance the session's cursor past everything read so far.
    #[allow(dead_code)] // SEAM-UNWIRED
    async fn commit_stream_offsets(&self, session: &StreamSession) -> Result<(), StudioError>;

    /// Drop the Studio-owned consumer group.
    #[allow(dead_code)] // SEAM-UNWIRED
    async fn close_stream_session(&self, session: &StreamSession) -> Result<(), StudioError>;

    /// Materialized views known to the cluster.
    #[allow(dead_code)] // SEAM-UNWIRED
    async fn materialized_views(&self) -> Result<Vec<MaterializedView>, StudioError>;

    /// Durable, replayable topics.
    #[allow(dead_code)] // SEAM-UNWIRED
    async fn topics(&self) -> Result<Vec<Topic>, StudioError>;

    /// Cron-style scheduled jobs.
    #[allow(dead_code)] // SEAM-UNWIRED
    async fn scheduled_jobs(&self) -> Result<Vec<ScheduledJob>, StudioError>;

    /// LISTEN/NOTIFY channels.
    #[allow(dead_code)] // SEAM-UNWIRED
    async fn notify_channels(&self) -> Result<Vec<NotifyChannel>, StudioError>;

    /// The pub/sub message tail across channels.
    #[allow(dead_code)] // SEAM-UNWIRED
    async fn notify_messages(&self) -> Result<Vec<NotifyMessage>, StudioError>;
}

/// Build display rows from the static mock change feed. Lives here (not in the
/// view) so the seam — not the screen — owns the native-doc → display mapping.
pub(crate) fn cdc_rows_from_mock() -> Vec<CdcRow> {
    use crate::data::mock;
    mock::cdc_events()
        .into_iter()
        .enumerate()
        .map(|(i, ev)| {
            let mut payload_json = sonic_rs::to_string(&ev.payload).unwrap_or_default();
            if let Some(note) = ev.note {
                payload_json.push(' ');
                payload_json.push_str(note);
            }
            let op = match ev.op {
                mock::ChangeOp::Insert => CdcOp::Insert,
                mock::ChangeOp::Update => CdcOp::Update,
                mock::ChangeOp::Delete => CdcOp::Delete,
            };
            CdcRow {
                id: format!("cdc-{i}"),
                time: ev.time.to_string(),
                op,
                collection: ev.collection.to_string(),
                payload_json,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::async_state::AsyncState;
    use crate::services::connection_service::MockConnectionService;

    #[test]
    fn mock_rows_have_unique_stable_ids() {
        let rows = cdc_rows_from_mock();
        assert!(!rows.is_empty());
        let mut ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count, "ids must be unique");
        assert_eq!(rows[0].id, "cdc-0");
    }

    // Each method gets its own empty/erroring pair rather than one combined
    // check per behaviour, mirroring `admin_data.rs`: `apply(self.behavior, mock::x)`
    // and a mis-wired `Ok(mock::x())` both satisfy a single shared assertion, so
    // every method needs its own proof that it actually reads `self.behavior`.

    #[tokio::test]
    async fn every_streams_list_read_has_unique_ids() {
        let svc = MockConnectionService::ready();
        let mvs = svc.materialized_views().await.expect("mvs");
        let topics = svc.topics().await.expect("topics");
        let jobs = svc.scheduled_jobs().await.expect("jobs");
        let channels = svc.notify_channels().await.expect("channels");
        let messages = svc.notify_messages().await.expect("messages");

        assert_unique(mvs.iter().map(|x| x.id.as_str()), "materialized_views");
        assert_unique(topics.iter().map(|x| x.id.as_str()), "topics");
        assert_unique(jobs.iter().map(|x| x.id.as_str()), "scheduled_jobs");
        assert_unique(channels.iter().map(|x| x.id.as_str()), "notify_channels");
        assert_unique(messages.iter().map(|x| x.id.as_str()), "notify_messages");
    }

    fn assert_unique<'a>(it: impl Iterator<Item = &'a str>, what: &str) {
        let mut v: Vec<&str> = it.collect();
        let total = v.len();
        assert!(total > 0, "{what} fixture must not be empty");
        v.sort_unstable();
        v.dedup();
        assert_eq!(total, v.len(), "{what} ids must be unique");
    }

    #[tokio::test]
    async fn materialized_views_empty_is_empty() {
        let svc = MockConnectionService::empty();
        let s = AsyncState::from_value(Some(svc.materialized_views().await));
        assert!(s.is_empty());
    }

    #[tokio::test]
    async fn materialized_views_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.materialized_views().await));
        assert!(s.error_message().is_some());
    }

    #[tokio::test]
    async fn topics_empty_is_empty() {
        let svc = MockConnectionService::empty();
        let s = AsyncState::from_value(Some(svc.topics().await));
        assert!(s.is_empty());
    }

    #[tokio::test]
    async fn topics_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.topics().await));
        assert!(s.error_message().is_some());
    }

    #[tokio::test]
    async fn scheduled_jobs_empty_is_empty() {
        let svc = MockConnectionService::empty();
        let s = AsyncState::from_value(Some(svc.scheduled_jobs().await));
        assert!(s.is_empty());
    }

    #[tokio::test]
    async fn scheduled_jobs_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.scheduled_jobs().await));
        assert!(s.error_message().is_some());
    }

    #[tokio::test]
    async fn notify_channels_empty_is_empty() {
        let svc = MockConnectionService::empty();
        let s = AsyncState::from_value(Some(svc.notify_channels().await));
        assert!(s.is_empty());
    }

    #[tokio::test]
    async fn notify_channels_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.notify_channels().await));
        assert!(s.error_message().is_some());
    }

    #[tokio::test]
    async fn notify_messages_empty_is_empty() {
        let svc = MockConnectionService::empty();
        let s = AsyncState::from_value(Some(svc.notify_messages().await));
        assert!(s.is_empty());
    }

    #[tokio::test]
    async fn notify_messages_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.notify_messages().await));
        assert!(s.error_message().is_some());
    }

    #[tokio::test]
    async fn scheduled_jobs_fixture_has_a_failed_and_a_successful_job() {
        let svc = MockConnectionService::ready();
        let jobs = svc.scheduled_jobs().await.expect("jobs");
        assert!(
            jobs.iter().any(|j| j.last_status == "success"),
            "fixture must include a successful job"
        );
        assert!(
            jobs.iter().any(|j| j.last_status.starts_with("failed")),
            "fixture must include a failed job so the status column isn't uniform"
        );
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use crate::services::connection_service::MockConnectionService;
    use crate::services::mock_behavior::MockBehavior;

    #[tokio::test]
    async fn session_group_is_studio_scoped() {
        let svc = MockConnectionService::ready();
        let s = svc.open_stream_session("cdc").await.expect("session opens");
        assert!(
            s.group.starts_with("studio_"),
            "Studio must use its own consumer group, got {}",
            s.group
        );
    }

    #[tokio::test]
    async fn open_stream_session_preserves_the_requested_stream_name() {
        let svc = MockConnectionService::ready();
        let s = svc.open_stream_session("cdc").await.expect("session opens");
        assert_eq!(s.stream, "cdc");
        assert_eq!(s.group, "studio_cdc");
    }

    /// Reads do not advance the cursor: two reads without a commit return the
    /// same rows. A naive poll loop would therefore repeat forever.
    #[tokio::test]
    async fn reads_are_idempotent_until_committed() {
        let svc = MockConnectionService::ready();
        let s = svc.open_stream_session("cdc").await.expect("session");
        let first = svc.cdc_batch(&s, 10).await.expect("first read");
        let second = svc.cdc_batch(&s, 10).await.expect("second read");
        assert_eq!(first, second, "an uncommitted re-read must be identical");
        assert!(!first.is_empty());
    }

    #[tokio::test]
    async fn commit_advances_past_the_batch() {
        let svc = MockConnectionService::ready();
        let s = svc.open_stream_session("cdc").await.expect("session");
        let first = svc.cdc_batch(&s, 10).await.expect("first read");
        svc.commit_stream_offsets(&s).await.expect("commit");
        let after = svc.cdc_batch(&s, 10).await.expect("read after commit");
        assert!(
            after.len() < first.len() || after.is_empty(),
            "commit must advance the cursor"
        );
    }

    #[tokio::test]
    async fn limit_is_respected() {
        let svc = MockConnectionService::ready();
        let s = svc.open_stream_session("cdc").await.expect("session");
        assert!(svc.cdc_batch(&s, 2).await.expect("read").len() <= 2);
    }

    /// `close_stream_session` drops the group by resetting the cursor, so a
    /// freshly opened session sees the same window a brand-new one would.
    #[tokio::test]
    async fn close_stream_session_resets_the_cursor_for_the_next_session() {
        let svc = MockConnectionService::ready();
        let s = svc.open_stream_session("cdc").await.expect("session");
        let first = svc.cdc_batch(&s, 10).await.expect("first read");
        svc.commit_stream_offsets(&s).await.expect("commit");
        svc.close_stream_session(&s).await.expect("close");

        let s2 = svc
            .open_stream_session("cdc")
            .await
            .expect("reopened session");
        let after_reopen = svc.cdc_batch(&s2, 10).await.expect("read after reopen");
        assert_eq!(
            first, after_reopen,
            "closing must drop the committed offset, not carry it forward"
        );
    }

    /// `commit_stream_offsets` and `close_stream_session` are not gated by the
    /// mock's behaviour switch and always succeed, even when reads fail. This is
    /// unlike `mark_all_read`, which is gated so its failure path is testable;
    /// these two have no UI call site yet, so nothing depends on them failing.
    #[tokio::test]
    async fn commit_and_close_succeed_even_when_reads_error() {
        let svc = MockConnectionService::erroring();
        let s = svc
            .open_stream_session("cdc")
            .await
            .expect("open is not a read");
        assert!(svc.cdc_batch(&s, 10).await.is_err(), "reads still fail");
        assert!(svc.commit_stream_offsets(&s).await.is_ok());
        assert!(svc.close_stream_session(&s).await.is_ok());
    }

    #[tokio::test]
    async fn cdc_batch_empty_behavior_returns_no_rows() {
        let svc = MockConnectionService::empty();
        let s = svc.open_stream_session("cdc").await.expect("session");
        let batch = svc.cdc_batch(&s, 10).await.expect("empty read is Ok");
        assert!(batch.is_empty());
    }

    #[tokio::test]
    async fn cdc_batch_erroring_behavior_is_err() {
        let svc = MockConnectionService::erroring();
        let s = svc.open_stream_session("cdc").await.expect("session");
        assert!(svc.cdc_batch(&s, 10).await.is_err());
    }

    /// Regression: `cdc_batch` used to record `cdc_read_end` from the
    /// computed batch *before* consulting `self.behavior`, so an erroring
    /// read still moved the cursor as if it had delivered every row. A
    /// following commit then promoted that phantom offset into
    /// `cdc_committed`, silently skipping events the caller never saw. This
    /// simulates the same session's connection recovering (shared cursor
    /// cells, `Erroring` swapped for `Ready`) and proves the events are
    /// still there to read.
    #[tokio::test]
    async fn erroring_read_then_commit_does_not_skip_events_once_reads_recover() {
        let erroring = MockConnectionService::erroring();
        let s = erroring
            .open_stream_session("cdc")
            .await
            .expect("open is not a read");
        assert!(
            erroring.cdc_batch(&s, 10).await.is_err(),
            "read fails as configured"
        );
        erroring
            .commit_stream_offsets(&s)
            .await
            .expect("commit always succeeds, even after a failed read");

        let recovered = erroring.with_shared_state(MockBehavior::Ready);
        let after = recovered
            .cdc_batch(&s, 10)
            .await
            .expect("the recovered read succeeds");
        assert!(
            !after.is_empty(),
            "a commit after a failed read must not have advanced the cursor \
             past events the caller never saw"
        );
    }

    /// Same regression as above, for the `Empty` behaviour: an empty read
    /// delivers zero rows, so a following commit must leave the cursor
    /// exactly where it was, not wherever the (discarded) full batch would
    /// have ended.
    #[tokio::test]
    async fn empty_read_then_commit_does_not_skip_events_once_reads_recover() {
        let empty = MockConnectionService::empty();
        let s = empty
            .open_stream_session("cdc")
            .await
            .expect("open is not a read");
        let first = empty.cdc_batch(&s, 10).await.expect("empty read is Ok");
        assert!(first.is_empty(), "Empty behaviour delivers no rows");
        empty
            .commit_stream_offsets(&s)
            .await
            .expect("commit always succeeds, even after an empty read");

        let recovered = empty.with_shared_state(MockBehavior::Ready);
        let after = recovered
            .cdc_batch(&s, 10)
            .await
            .expect("the recovered read succeeds");
        assert!(
            !after.is_empty(),
            "a commit after an empty read must not have advanced the cursor \
             past events the caller never saw"
        );
    }
}
