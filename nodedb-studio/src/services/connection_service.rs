//! The backend seam.
//!
//! Everything the UI needs from "the outside world" goes through this trait.
//! Today the only implementor is `MockConnectionService`, reading
//! `crate::data::mock`. When NodeDB's real client lands, a second implementor
//! wraps it here — and async (`use_resource`) is introduced at this boundary,
//! not sprinkled through the views.

use async_trait::async_trait;

use crate::models::connections::{ConnectionDraft, ConnectionEvent, ConnectionTest};
use crate::models::notification::Notification;
use crate::models::shell::{NavBadges, RecentItem, SessionInfo};
use crate::services::error::StudioError;
use crate::state::connection::ActiveConnection;
use crate::state::connections_registry::{Credentials, SavedConnection};

/// Async because the real client talks to NodeDB over the network. The Dioxus
/// runtime is single-threaded, so `?Send` is correct (and `use_resource` has no
/// `Send` bound). Every method returns `Result<_, StudioError>`: the mock can
/// fail `connect` (offline or unknown name), but the real impl surfaces real
/// errors through the same channel.
#[async_trait(?Send)]
pub trait ConnectionService {
    /// The saved-connection registry backing the Connection Manager.
    async fn list_connections(&self) -> Result<Vec<SavedConnection>, StudioError>;

    /// The connection activity log, newest first.
    async fn connection_activity(&self) -> Result<Vec<ConnectionEvent>, StudioError>;

    /// The full notification feed, newest first (capability gating happens
    /// at render time).
    async fn notifications(&self) -> Result<Vec<Notification>, StudioError>;

    /// What the user opened lately, newest first, for the command palette.
    async fn recent_items(&self) -> Result<Vec<RecentItem>, StudioError>;

    /// Open a session by saved-connection name using an explicit identity.
    /// `StudioError::MissingUsername` if the username is blank;
    /// `StudioError::NotConnected` if the name is unknown; a connection
    /// error if the server is offline.
    async fn connect(
        &self,
        name: &str,
        creds: &Credentials,
    ) -> Result<ActiveConnection, StudioError>;

    /// Reach the server a draft describes and report what it saw. Nothing is
    /// saved and no session opens: Test never persists credentials.
    async fn test_connection(
        &self,
        draft: &ConnectionDraft,
        creds: &Credentials,
    ) -> Result<ConnectionTest, StudioError>;

    /// Save a connection. `replacing` names the entry an edit overwrites;
    /// `None` adds a new one. `creds.password` goes to the OS keychain when
    /// `draft.keychain` is set and nowhere else. `StudioError::NameTaken`
    /// when another entry has the name.
    async fn save_connection(
        &self,
        draft: ConnectionDraft,
        creds: Credentials,
        replacing: Option<String>,
    ) -> Result<SavedConnection, StudioError>;

    /// Save a copy of `name` under a free name, and return it.
    async fn duplicate_connection(&self, name: &str) -> Result<SavedConnection, StudioError>;

    /// Mark every notification read. A seam write: the real client persists this
    /// server-side; the mock persists it in-process so the unread badge does not
    /// revert on reload.
    async fn mark_all_read(&self) -> Result<(), StudioError>;

    /// Mark one notification read.
    async fn mark_read(&self, id: &str) -> Result<(), StudioError>;

    /// Badge counts for the sidebar's Query entry.
    async fn nav_badges(&self) -> Result<NavBadges, StudioError>;

    /// Server version of the active session, for the footer.
    async fn session_info(&self) -> Result<SessionInfo, StudioError>;

    /// All databases visible on the active connection.
    async fn databases(&self) -> Result<Vec<String>, StudioError>;

    /// Move the session to another database of the active connection. This
    /// changes the catalog context, never the connection.
    async fn select_database(&self, name: &str) -> Result<(), StudioError>;
}
