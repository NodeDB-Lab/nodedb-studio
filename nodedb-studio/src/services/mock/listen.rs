//! LISTEN/NOTIFY: channels, a listening session, and sends.

use async_trait::async_trait;

use crate::models::listen::{ListenSession, NotifyChannel, NotifyMessage};
use crate::services::error::StudioError;
use crate::services::listen_data::ListenData;
use crate::services::mock::MockConnectionService;
use crate::services::mock_behavior::{apply, apply_one_result, apply_write};

#[async_trait(?Send)]
impl ListenData for MockConnectionService {
    async fn notify_channels(&self) -> Result<Vec<NotifyChannel>, StudioError> {
        apply(self.behavior, || self.listen.borrow().channels()).await
    }

    async fn start_listening(&self, channel: &str) -> Result<ListenSession, StudioError> {
        apply_write(self.behavior, || self.listen.borrow_mut().listen(channel)).await
    }

    async fn stop_listening(&self, session: &ListenSession) -> Result<(), StudioError> {
        self.listen.borrow_mut().unlisten(session);
        Ok(())
    }

    async fn poll_notifications(
        &self,
        session: &ListenSession,
    ) -> Result<Vec<NotifyMessage>, StudioError> {
        let polled = self.listen.borrow_mut().poll(session);
        apply_one_result(self.behavior, polled).await
    }

    async fn send_notification(&self, channel: &str, payload: &str) -> Result<usize, StudioError> {
        apply_write(self.behavior, || {
            self.listen.borrow_mut().send(channel, payload)
        })
        .await
    }
}
