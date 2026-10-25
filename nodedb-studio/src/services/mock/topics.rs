//! Durable topics: reads, create, publish, and the offset reset.

use async_trait::async_trait;

use crate::models::topics::{
    ConsumerGroup, MessageRange, NewTopic, PublishDraft, ResetPreview, ResetScope, ResetTarget,
    Topic, TopicMessage,
};
use crate::services::error::StudioError;
use crate::services::mock::MockConnectionService;
use crate::services::mock_behavior::{apply, apply_one_result, apply_result, apply_write};
use crate::services::topic_data::TopicData;

#[async_trait(?Send)]
impl TopicData for MockConnectionService {
    async fn topics(&self) -> Result<Vec<Topic>, StudioError> {
        apply(self.behavior, || self.topics.borrow().topics()).await
    }

    async fn consumer_groups(&self, topic: &str) -> Result<Vec<ConsumerGroup>, StudioError> {
        let read = self.topics.borrow().groups(topic);
        apply_result(self.behavior, read).await
    }

    async fn topic_messages(
        &self,
        topic: &str,
        partition: u32,
        from: u64,
        to: u64,
    ) -> Result<MessageRange, StudioError> {
        let read = self.topics.borrow().read(topic, partition, from, to);
        // An empty server answers with an empty range, not an error.
        match (self.behavior, read) {
            (crate::services::mock_behavior::MockBehavior::Empty, Ok(mut r)) => {
                r.messages.clear();
                Ok(r)
            }
            (behavior, read) => apply_one_result(behavior, read).await,
        }
    }

    async fn create_topic(&self, form: &NewTopic) -> Result<Topic, StudioError> {
        apply_write(self.behavior, || self.topics.borrow_mut().create(form)).await
    }

    async fn publish_message(
        &self,
        topic: &str,
        draft: &PublishDraft,
    ) -> Result<TopicMessage, StudioError> {
        apply_write(self.behavior, || {
            self.topics.borrow_mut().publish(topic, draft)
        })
        .await
    }

    async fn preview_offset_reset(
        &self,
        group: &str,
        scope: ResetScope,
        target: ResetTarget,
    ) -> Result<ResetPreview, StudioError> {
        let preview = self.topics.borrow().preview_reset(group, scope, target);
        apply_one_result(self.behavior, preview).await
    }

    async fn apply_offset_reset(
        &self,
        group: &str,
        scope: ResetScope,
        target: ResetTarget,
    ) -> Result<ConsumerGroup, StudioError> {
        apply_write(self.behavior, || {
            self.topics.borrow_mut().apply_reset(group, scope, target)
        })
        .await
    }
}
