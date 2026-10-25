//! Connections, notifications, and session reads and writes.

use async_trait::async_trait;

use crate::models::connections::{ConnectionDraft, ConnectionEvent, ConnectionTest};
use crate::models::notification::Notification;
use crate::models::shell::{NavBadges, RecentItem, SessionInfo};
use crate::services::connection_service::ConnectionService;
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;
use crate::state::connection::ActiveConnection;
use crate::state::connections_registry::{Credentials, SavedConnection};

#[async_trait(?Send)]
impl ConnectionService for NodeDbConnectionService {
    async fn list_connections(&self) -> Result<Vec<SavedConnection>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn connection_activity(&self) -> Result<Vec<ConnectionEvent>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn notifications(&self) -> Result<Vec<Notification>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn recent_items(&self) -> Result<Vec<RecentItem>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn connect(
        &self,
        _name: &str,
        _creds: &Credentials,
    ) -> Result<ActiveConnection, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn test_connection(
        &self,
        _draft: &ConnectionDraft,
        _creds: &Credentials,
    ) -> Result<ConnectionTest, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn save_connection(
        &self,
        _draft: ConnectionDraft,
        _creds: Credentials,
        _replacing: Option<String>,
    ) -> Result<SavedConnection, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn duplicate_connection(&self, _name: &str) -> Result<SavedConnection, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn mark_all_read(&self) -> Result<(), StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn mark_read(&self, _id: &str) -> Result<(), StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn nav_badges(&self) -> Result<NavBadges, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn session_info(&self) -> Result<SessionInfo, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn databases(&self) -> Result<Vec<String>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn select_database(&self, _name: &str) -> Result<(), StudioError> {
        Err(StudioError::NotConnected)
    }
}
