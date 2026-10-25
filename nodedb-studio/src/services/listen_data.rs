//! LISTEN/NOTIFY at the backend seam. A listening session is Studio's own:
//! stopping it never affects another session, and nothing here replays.

use async_trait::async_trait;

use crate::models::listen::{ListenSession, NotifyChannel, NotifyMessage};
use crate::services::error::StudioError;

#[async_trait(?Send)]
pub trait ListenData {
    /// The channels Studio can listen on.
    async fn notify_channels(&self) -> Result<Vec<NotifyChannel>, StudioError>;

    /// Start a Studio listening session on `channel`.
    async fn start_listening(&self, channel: &str) -> Result<ListenSession, StudioError>;

    /// End the session. Other sessions on the channel keep listening.
    async fn stop_listening(&self, session: &ListenSession) -> Result<(), StudioError>;

    /// The notifications that arrived since the last poll, oldest first.
    async fn poll_notifications(
        &self,
        session: &ListenSession,
    ) -> Result<Vec<NotifyMessage>, StudioError>;

    /// Send `payload` (JSON) to every session listening on `channel`. Returns
    /// how many sessions received it.
    async fn send_notification(&self, channel: &str, payload: &str) -> Result<usize, StudioError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::async_state::AsyncState;
    use crate::services::mock::MockConnectionService;

    #[tokio::test]
    async fn channel_names_are_unique() {
        let svc = MockConnectionService::ready();
        let channels = svc.notify_channels().await.expect("channels");
        let mut names: Vec<_> = channels.iter().map(|c| c.name.as_str()).collect();
        let n = names.len();
        names.sort_unstable();
        names.dedup();
        assert!(n > 0);
        assert_eq!(names.len(), n);
    }

    #[tokio::test]
    async fn empty_reaches_the_empty_state() {
        let svc = MockConnectionService::empty();
        assert!(AsyncState::from_value(Some(svc.notify_channels().await)).is_empty());
    }

    #[tokio::test]
    async fn erroring_reaches_the_error_state() {
        let svc = MockConnectionService::erroring();
        assert!(
            AsyncState::from_value(Some(svc.notify_channels().await))
                .error_message()
                .is_some()
        );
        assert!(svc.start_listening("user_events").await.is_err());
    }

    #[tokio::test]
    async fn a_listener_receives_what_is_sent_after_it_starts() {
        let svc = MockConnectionService::ready();
        let s = svc.start_listening("order_updates").await.expect("listens");
        assert_eq!(s.id, 12);
        assert_eq!(
            svc.send_notification("order_updates", "{\"id\": 7}")
                .await
                .expect("sends"),
            1
        );
        let got = svc.poll_notifications(&s).await.expect("polls");
        assert_eq!(got.len(), 1);
        svc.stop_listening(&s).await.expect("stops");
        assert_eq!(
            svc.send_notification("order_updates", "{}")
                .await
                .expect("sends"),
            0
        );
        assert!(svc.poll_notifications(&s).await.is_err());
    }

    #[tokio::test]
    async fn a_payload_that_is_not_json_is_rejected() {
        let svc = MockConnectionService::ready();
        assert!(matches!(
            svc.send_notification("user_events", "{ nope").await,
            Err(StudioError::Rejected { .. })
        ));
    }
}
