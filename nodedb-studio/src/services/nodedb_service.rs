//! The real-client-backed seam impl. Phase 1: inert stub.
//!
//! Holds an `Option<NativeClient>` (None until Phase 2 connects). While the
//! client is None, every method returns `StudioError::NotConnected` — never
//! a panic or todo!(). Phase 2 (CONN-01..07) fills the client via
//! ConnectionBuilder; that wiring is explicitly out of scope here.

use async_trait::async_trait;
use nodedb_client::NativeClient;

use crate::models::notification::Notification;
use crate::services::connection_service::ConnectionService;
use crate::services::error::StudioError;
use crate::state::connection::ActiveConnection;
use crate::state::connections_registry::SavedConnection;

#[derive(Default)]
pub struct NodeDbConnectionService {
    /// None until a real connection is opened in Phase 2 (ConnectionBuilder
    /// fills this). The field exists now so the seam type is forward-compatible.
    #[allow(dead_code)]
    client: Option<NativeClient>,
}

#[async_trait(?Send)]
impl ConnectionService for NodeDbConnectionService {
    async fn list_connections(&self) -> Result<Vec<SavedConnection>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn notifications(&self) -> Result<Vec<Notification>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn connect(&self, _name: &str) -> Result<ActiveConnection, StudioError> {
        Err(StudioError::NotConnected)
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::*;

    #[tokio::test]
    async fn stub_returns_not_connected() {
        let svc = NodeDbConnectionService::default();
        assert!(matches!(
            svc.list_connections().await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.notifications().await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.connect("anything").await,
            Err(StudioError::NotConnected)
        ));
    }

    #[test]
    fn stub_is_object_safe_behind_rc() {
        // Compile-time guarantee: the stub coerces to the seam trait object,
        // exactly as `app.rs` provides it via context.
        let _s: Rc<dyn ConnectionService> = Rc::new(NodeDbConnectionService::default());
    }
}
