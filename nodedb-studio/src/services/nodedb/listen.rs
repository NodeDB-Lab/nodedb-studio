//! LISTEN/NOTIFY against a real server. Not wired yet.

use async_trait::async_trait;

use crate::models::listen::{ListenSession, NotifyChannel, NotifyMessage};
use crate::services::error::StudioError;
use crate::services::listen_data::ListenData;
use crate::services::nodedb::NodeDbConnectionService;

#[async_trait(?Send)]
impl ListenData for NodeDbConnectionService {
    async fn notify_channels(&self) -> Result<Vec<NotifyChannel>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn start_listening(&self, _channel: &str) -> Result<ListenSession, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn stop_listening(&self, _session: &ListenSession) -> Result<(), StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn poll_notifications(
        &self,
        _session: &ListenSession,
    ) -> Result<Vec<NotifyMessage>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn send_notification(
        &self,
        _channel: &str,
        _payload: &str,
    ) -> Result<usize, StudioError> {
        Err(StudioError::NotConnected)
    }
}
