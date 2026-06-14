//! The backend seam.
//!
//! Everything the UI needs from "the outside world" goes through this trait.
//! Today the only implementor is `MockConnectionService`, reading
//! `crate::data::mock`. When NodeDB's real client lands, a second implementor
//! wraps it here — and async (`use_resource`) is introduced at this boundary,
//! not sprinkled through the views.

use async_trait::async_trait;

use crate::data::mock;
use crate::models::notification::Notification;
use crate::services::error::StudioError;
use crate::state::connection::ActiveConnection;
use crate::state::connections_registry::SavedConnection;

/// Async because the real client talks to NodeDB over the network. The Dioxus
/// runtime is single-threaded, so `?Send` is correct (and `use_resource` has no
/// `Send` bound). Every method returns `Result<_, StudioError>`: the mock can
/// only fail `connect` (unknown/offline name -> `NotConnected`), but the real
/// impl surfaces real failures through the same channel.
#[async_trait(?Send)]
pub trait ConnectionService {
    /// The saved-connection registry backing the Connection Manager.
    async fn list_connections(&self) -> Result<Vec<SavedConnection>, StudioError>;

    /// The full notification feed (capability gating happens at render time).
    async fn notifications(&self) -> Result<Vec<Notification>, StudioError>;

    /// Open a session by saved-connection name. `StudioError::NotConnected` if
    /// the name is unknown or the connection is offline.
    async fn connect(&self, name: &str) -> Result<ActiveConnection, StudioError>;
}

/// Hardcoded implementation used by the skeleton. Data is identical to before;
/// the methods are merely `async` now (and resolve instantly).
#[derive(Debug, Clone, Copy, Default)]
pub struct MockConnectionService;

#[async_trait(?Send)]
impl ConnectionService for MockConnectionService {
    async fn list_connections(&self) -> Result<Vec<SavedConnection>, StudioError> {
        Ok(mock::connections())
    }

    async fn notifications(&self) -> Result<Vec<Notification>, StudioError> {
        Ok(mock::notifications())
    }

    async fn connect(&self, name: &str) -> Result<ActiveConnection, StudioError> {
        mock::connections()
            .into_iter()
            .find(|c| c.name == name)
            .and_then(|c| c.open())
            .ok_or(StudioError::NotConnected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_notifications_returns_data() {
        let svc = MockConnectionService;
        let notifs = svc
            .notifications()
            .await
            .expect("mock notifications are infallible");
        assert!(!notifs.is_empty());
    }

    #[tokio::test]
    async fn mock_list_connections_returns_data() {
        let svc = MockConnectionService;
        let conns = svc
            .list_connections()
            .await
            .expect("mock list_connections is infallible");
        assert!(!conns.is_empty());
    }

    #[tokio::test]
    async fn mock_connect_known_name_returns_session() {
        let svc = MockConnectionService;
        // `staging-cluster` is a connectable Online mock connection (data/mock.rs).
        let session = svc.connect("staging-cluster").await;
        assert!(session.is_ok());
    }

    #[tokio::test]
    async fn mock_connect_unknown_name_is_not_connected() {
        let svc = MockConnectionService;
        assert!(matches!(
            svc.connect("does-not-exist").await,
            Err(StudioError::NotConnected)
        ));
    }
}
