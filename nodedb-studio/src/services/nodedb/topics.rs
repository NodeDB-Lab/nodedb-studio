//! Durable topics against a real server. Not wired yet.

use async_trait::async_trait;

use crate::models::topics::{
    ConsumerGroup, MessageRange, NewTopic, PublishDraft, ResetPreview, ResetScope, ResetTarget,
    Topic, TopicMessage,
};
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;
use crate::services::topic_data::TopicData;

#[async_trait(?Send)]
impl TopicData for NodeDbConnectionService {
    async fn topics(&self) -> Result<Vec<Topic>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn consumer_groups(&self, _topic: &str) -> Result<Vec<ConsumerGroup>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn topic_messages(
        &self,
        _topic: &str,
        _partition: u32,
        _from: u64,
        _to: u64,
    ) -> Result<MessageRange, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn create_topic(&self, _form: &NewTopic) -> Result<Topic, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn publish_message(
        &self,
        _topic: &str,
        _draft: &PublishDraft,
    ) -> Result<TopicMessage, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn preview_offset_reset(
        &self,
        _group: &str,
        _scope: ResetScope,
        _target: ResetTarget,
    ) -> Result<ResetPreview, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn apply_offset_reset(
        &self,
        _group: &str,
        _scope: ResetScope,
        _target: ResetTarget,
    ) -> Result<ConsumerGroup, StudioError> {
        Err(StudioError::NotConnected)
    }
}
