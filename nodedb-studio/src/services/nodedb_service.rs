//! The real-client-backed `ConnectionService` impl. Currently an inert stub:
//! every method returns `StudioError::NotConnected` until a real connection is
//! opened — no panic or `todo!()` in the interim.
//!
//! The future `ConnectionBuilder` wiring will add the held client (an
//! `Option<NativeClient>`, gated behind nodedb-client's `native` feature) in the
//! same change that first reads it. That feature pulls a TLS + C-toolchain build
//! tree (tokio-rustls, aws-lc-rs, cmake), so it is deliberately left disabled
//! until a live consumer exists rather than carried here for a field nothing reads.

use async_trait::async_trait;

use crate::models::notification::Notification;
use crate::services::connection_service::ConnectionService;
use crate::services::error::StudioError;
use crate::state::connection::ActiveConnection;
use crate::state::connections_registry::SavedConnection;

// The Phase-2 seam impl: its trait conformance and object-safety are proven by
// the tests below, but the mock is still the active injected service, so this
// type is not constructed in non-test code yet. The phase that wires it in as
// the real service removes this allow.
#[allow(dead_code)]
#[derive(Default)]
pub struct NodeDbConnectionService;

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
        let svc = NodeDbConnectionService;
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
        let _s: Rc<dyn ConnectionService> = Rc::new(NodeDbConnectionService);
    }
}
