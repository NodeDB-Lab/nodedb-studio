//! The backend seam.
//!
//! Everything the UI needs from "the outside world" goes through this trait.
//! Today the only implementor is `MockConnectionService`, reading
//! `crate::data::mock`. When NodeDB's real client lands, a second implementor
//! wraps it here — and async (`use_resource`) is introduced at this boundary,
//! not sprinkled through the views.

use crate::data::mock;
use crate::models::notification::Notification;
use crate::state::connection::ActiveConnection;
use crate::state::connections_registry::SavedConnection;

pub trait ConnectionService {
    /// The saved-connection registry backing the Connection Manager.
    fn list_connections(&self) -> Vec<SavedConnection>;

    /// The full notification feed (capability gating happens at render time).
    fn notifications(&self) -> Vec<Notification>;

    /// Open a session by saved-connection name. `None` if the name is unknown
    /// or the connection is offline.
    fn connect(&self, name: &str) -> Option<ActiveConnection>;
}

/// Synchronous, hardcoded implementation used by the skeleton.
#[derive(Debug, Clone, Copy, Default)]
pub struct MockConnectionService;

impl ConnectionService for MockConnectionService {
    fn list_connections(&self) -> Vec<SavedConnection> {
        mock::connections()
    }

    fn notifications(&self) -> Vec<Notification> {
        mock::notifications()
    }

    fn connect(&self, name: &str) -> Option<ActiveConnection> {
        mock::connections()
            .into_iter()
            .find(|c| c.name == name)
            .and_then(|c| c.open())
    }
}
