//! The studio's typed error model, mapped from nodedb-client's NodeDbError.
//!
//! This is the seam's Result error. Categorized so views can branch on
//! category (Connection/Auth/...) and show tailored messages + a Retry
//! affordance gated on `is_retriable()`.

use nodedb_client::NodeDbError;
use nodedb_types::error::ErrorDetails;
use thiserror::Error;

// The seam's Result error: returned by every `ConnectionService` method and
// consumed by the views in later plans (01-03..04).
#[derive(Debug, Error)]
pub enum StudioError {
    #[error("connection error: {0}")]
    Connection(#[source] NodeDbError),
    #[error("authentication error: {0}")]
    Auth(#[source] NodeDbError),
    #[error("not found: {0}")]
    NotFound(#[source] NodeDbError),
    #[error("conflict: {0}")]
    Conflict(#[source] NodeDbError),
    #[error("read-only: {0}")]
    ReadOnly(#[source] NodeDbError),
    #[error("setup error: {0}")]
    Setup(#[source] NodeDbError),
    #[error("server error: {0}")]
    Server(#[source] NodeDbError),
    #[error("not connected to a database")]
    NotConnected,
}

impl StudioError {
    /// Drives the Retry affordance. Delegates to the wrapped NodeDbError;
    /// `NotConnected` is never retriable (it is studio-originated, not transient).
    pub fn is_retriable(&self) -> bool {
        match self {
            StudioError::NotConnected => false,
            StudioError::Connection(e)
            | StudioError::Auth(e)
            | StudioError::NotFound(e)
            | StudioError::Conflict(e)
            | StudioError::ReadOnly(e)
            | StudioError::Setup(e)
            | StudioError::Server(e) => e.is_retriable(),
        }
    }
}

impl From<NodeDbError> for StudioError {
    fn from(e: NodeDbError) -> Self {
        // Match on the BORROWED details to pick a category, then move `e` in.
        // ErrorDetails is foreign and #[non_exhaustive] -> the `_` arm is
        // REQUIRED by the compiler. This is NOT a violation of the studio's
        // "no `_ =>` on exhaustive domain enums" rule (that rule governs the
        // studio's OWN enums, e.g. StudioError above, which has no `_`).
        use ErrorDetails as D;
        match e.details() {
            D::HandshakeFailed { .. }
            | D::SyncConnectionFailed
            | D::NodeUnreachable
            | D::NoLeader
            | D::NotLeader { .. }
            | D::MigrationInProgress
            | D::ServerOverload
            | D::MemoryExhausted { .. } => StudioError::Connection(e),

            D::AuthorizationDenied { .. } | D::AuthExpired => StudioError::Auth(e),

            D::CollectionNotFound { .. }
            | D::DocumentNotFound { .. }
            | D::CollectionDraining { .. }
            | D::CollectionDeactivated { .. } => StudioError::NotFound(e),

            D::WriteConflict { .. }
            | D::ConstraintViolation { .. }
            | D::PrevalidationRejected { .. }
            | D::BalanceViolation { .. }
            | D::StateTransitionViolation { .. }
            | D::TransitionCheckViolation { .. }
            | D::TypeGuardViolation { .. }
            | D::InsufficientBalance { .. }
            | D::Overflow { .. }
            | D::TypeMismatch { .. } => StudioError::Conflict(e),

            D::MirrorReadOnly { .. }
            | D::AppendOnlyViolation { .. }
            | D::PeriodLocked { .. }
            | D::LegalHoldActive { .. }
            | D::RetentionViolation { .. }
            | D::MirrorNotPromoted { .. } => StudioError::ReadOnly(e),

            D::Config | D::BadRequest | D::SqlNotEnabled => StudioError::Setup(e),

            // Foreign #[non_exhaustive] enum: REQUIRED catch-all -> Server.
            _ => StudioError::Server(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodedb_client::NodeDbError;

    #[test]
    fn maps_collection_not_found_to_not_found() {
        assert!(matches!(
            StudioError::from(NodeDbError::collection_not_found("users")),
            StudioError::NotFound(_)
        ));
    }

    #[test]
    fn maps_document_not_found_to_not_found() {
        assert!(matches!(
            StudioError::from(NodeDbError::document_not_found("c", "id")),
            StudioError::NotFound(_)
        ));
    }

    #[test]
    fn maps_write_conflict_to_conflict_and_retriable() {
        let s = StudioError::from(NodeDbError::write_conflict("orders", "id1"));
        assert!(matches!(s, StudioError::Conflict(_)));
        assert!(s.is_retriable());
    }

    #[test]
    fn maps_authorization_denied_to_auth() {
        assert!(matches!(
            StudioError::from(NodeDbError::authorization_denied("res")),
            StudioError::Auth(_)
        ));
    }

    #[test]
    fn maps_auth_expired_to_auth() {
        assert!(matches!(
            StudioError::from(NodeDbError::auth_expired("x")),
            StudioError::Auth(_)
        ));
    }

    #[test]
    fn maps_mirror_read_only_to_read_only() {
        assert!(matches!(
            StudioError::from(NodeDbError::mirror_read_only("db")),
            StudioError::ReadOnly(_)
        ));
    }

    #[test]
    fn maps_bad_request_to_setup() {
        assert!(matches!(
            StudioError::from(NodeDbError::bad_request("x")),
            StudioError::Setup(_)
        ));
    }

    #[test]
    fn maps_config_to_setup() {
        assert!(matches!(
            StudioError::from(NodeDbError::config("x")),
            StudioError::Setup(_)
        ));
    }

    #[test]
    fn maps_node_unreachable_to_connection_and_retriable() {
        let s = StudioError::from(NodeDbError::node_unreachable("x"));
        assert!(matches!(s, StudioError::Connection(_)));
        assert!(s.is_retriable());
    }

    #[test]
    fn maps_internal_to_server() {
        assert!(matches!(
            StudioError::from(NodeDbError::internal("x")),
            StudioError::Server(_)
        ));
    }

    #[test]
    fn maps_storage_to_server() {
        assert!(matches!(
            StudioError::from(NodeDbError::storage("x")),
            StudioError::Server(_)
        ));
    }

    #[test]
    fn not_connected_is_not_retriable() {
        assert!(!StudioError::NotConnected.is_retriable());
    }
}
