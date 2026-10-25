//! Durable topics at the backend seam: topics, bounded message reads,
//! consumer groups, and the review-before-apply offset reset.

use async_trait::async_trait;

use crate::models::topics::{
    ConsumerGroup, MessageRange, NewTopic, PublishDraft, ResetPreview, ResetScope, ResetTarget,
    Topic, TopicMessage,
};
use crate::services::error::StudioError;

#[async_trait(?Send)]
pub trait TopicData {
    /// The durable topics of the selected database.
    async fn topics(&self) -> Result<Vec<Topic>, StudioError>;

    /// The consumer groups of `topic`, with committed offsets per partition.
    async fn consumer_groups(&self, topic: &str) -> Result<Vec<ConsumerGroup>, StudioError>;

    /// The retained messages of one partition in `from..=to`, newest first.
    /// A range wider than `MAX_READ` offsets is `StudioError::Rejected`.
    async fn topic_messages(
        &self,
        topic: &str,
        partition: u32,
        from: u64,
        to: u64,
    ) -> Result<MessageRange, StudioError>;

    /// Create a durable topic and return it with its stable id.
    async fn create_topic(&self, form: &NewTopic) -> Result<Topic, StudioError>;

    /// Append a message. Without a partition, the key picks one.
    async fn publish_message(
        &self,
        topic: &str,
        draft: &PublishDraft,
    ) -> Result<TopicMessage, StudioError>;

    /// What a reset would do to each partition, without applying it.
    async fn preview_offset_reset(
        &self,
        group: &str,
        scope: ResetScope,
        target: ResetTarget,
    ) -> Result<ResetPreview, StudioError>;

    /// Apply a reset and return the group with its new offsets.
    async fn apply_offset_reset(
        &self,
        group: &str,
        scope: ResetScope,
        target: ResetTarget,
    ) -> Result<ConsumerGroup, StudioError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::async_state::AsyncState;
    use crate::services::mock::MockConnectionService;

    #[tokio::test]
    async fn topic_ids_are_unique() {
        let svc = MockConnectionService::ready();
        let topics = svc.topics().await.expect("topics");
        let mut ids: Vec<_> = topics.iter().map(|t| t.id.as_str()).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert!(n > 0);
        assert_eq!(ids.len(), n);
    }

    #[tokio::test]
    async fn empty_reaches_the_empty_state() {
        let svc = MockConnectionService::empty();
        assert!(AsyncState::from_value(Some(svc.topics().await)).is_empty());
    }

    #[tokio::test]
    async fn erroring_reaches_the_error_state() {
        let svc = MockConnectionService::erroring();
        assert!(
            AsyncState::from_value(Some(svc.topics().await))
                .error_message()
                .is_some()
        );
        assert!(svc.consumer_groups("topic_01").await.is_err());
        assert!(svc.topic_messages("topic_01", 1, 90, 120).await.is_err());
    }

    #[tokio::test]
    async fn group_ids_are_unique_across_topics() {
        let svc = MockConnectionService::ready();
        let mut ids = Vec::new();
        for t in svc.topics().await.expect("topics") {
            ids.extend(
                svc.consumer_groups(&t.id)
                    .await
                    .expect("groups")
                    .into_iter()
                    .map(|g| g.id),
            );
        }
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n);
    }

    #[tokio::test]
    async fn a_wide_range_is_rejected() {
        let svc = MockConnectionService::ready();
        assert!(matches!(
            svc.topic_messages("topic_01", 1, 0, 900).await,
            Err(StudioError::Rejected { .. })
        ));
    }

    #[tokio::test]
    async fn a_reset_changes_nothing_until_it_is_applied() {
        let svc = MockConnectionService::ready();
        let before = svc.consumer_groups("topic_01").await.expect("groups");
        let preview = svc
            .preview_offset_reset(
                "grp_billing",
                ResetScope::Partition(1),
                ResetTarget::Earliest,
            )
            .await
            .expect("previews");
        assert_eq!(preview.replayed(), 21);
        assert_eq!(
            before,
            svc.consumer_groups("topic_01").await.expect("groups")
        );
        svc.apply_offset_reset(
            "grp_billing",
            ResetScope::Partition(1),
            ResetTarget::Earliest,
        )
        .await
        .expect("applies");
        let after = svc.consumer_groups("topic_01").await.expect("groups");
        assert_eq!(after[0].offsets[1].committed, 80);
        assert_eq!(after[0].offsets[0].committed, 98);
    }

    #[tokio::test]
    async fn a_created_topic_and_a_published_message_are_read_back() {
        let svc = MockConnectionService::ready();
        let mut form = NewTopic::blank();
        form.name = "audit_events".into();
        let t = svc.create_topic(&form).await.expect("creates");
        assert!(
            svc.topics()
                .await
                .expect("topics")
                .iter()
                .any(|x| x.id == t.id)
        );
        let m = svc
            .publish_message(
                &t.id,
                &PublishDraft {
                    partition: Some(0),
                    key: "k".into(),
                    value: "{\"a\": 1}".into(),
                },
            )
            .await
            .expect("publishes");
        let r = svc.topic_messages(&t.id, 0, 0, 10).await.expect("reads");
        assert_eq!(r.messages, vec![m]);
    }
}
