//! The studio's typed error model, mapped from nodedb-client's NodeDbError.
//!
//! This is the seam's Result error. Categorized so views can branch on
//! category (Connection/Auth/...) and show tailored messages + a Retry
//! affordance gated on `is_retriable()`.

use nodedb_client::NodeDbError;
use nodedb_types::error::ErrorDetails;
use thiserror::Error;

// The seam's Result error: returned by every `ConnectionService` method and
// consumed by the views. `Clone` so a `Resource` read can be cloned out of its
// guard and mapped through `AsyncState::from_value` (the canonical render path);
// `NodeDbError` is itself `Clone`, so this is a cheap, lossless derive.
#[derive(Debug, Clone, Error)]
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
    /// A result set did not carry the columns the decoder needs. Most often the
    /// server answered a `SHOW` with its session-variable fallback
    /// (`cols=["setting"]`), which would otherwise read as an empty screen.
    #[error("unexpected result columns: expected [{expected}], got [{got}]")]
    UnexpectedColumns { expected: String, got: String },
    /// A result's rows disagree with its header: a row whose width differs
    /// from the column count, or two rows sharing a key. Rendered as-is, the
    /// values would sit under the wrong headings or reuse the wrong row node.
    #[error("result rows do not match their header: {reason}")]
    MalformedRows { reason: String },
    /// Connect was attempted without an explicit username.
    #[error("a username is required to connect")]
    MissingUsername,
    /// A save would give two saved connections one name.
    #[error("a connection named `{0}` already exists: choose another name")]
    NameTaken(String),
    /// A write named a saved connection that is not in the registry.
    #[error("no saved connection named `{0}`")]
    UnknownConnection(String),
    /// A database switch named a database the connection does not list.
    #[error("the connection has no database named `{0}`")]
    UnknownDatabase(String),
    /// A record write named a revision the server no longer holds: someone
    /// else wrote the record first. The editor keeps the edit and offers the
    /// stored record.
    #[error(
        "revision conflict on {collection}/{id}: you edited revision {expected}, the server holds revision {stored}"
    )]
    RevisionConflict {
        collection: String,
        id: String,
        expected: u64,
        stored: u64,
    },
    /// A write or read named a record the collection does not hold.
    #[error("{collection}/{id} does not exist")]
    RecordMissing { collection: String, id: String },
    /// The session lacks the privilege a write needs.
    #[error("{privilege} permission required on {collection}")]
    PermissionDenied {
        privilege: &'static str,
        collection: String,
    },
    /// The server's own validation refused a write.
    #[error("the server rejected the write: {reason}")]
    Rejected { reason: String },
}

impl StudioError {
    /// Drives the Retry affordance. Delegates to the wrapped NodeDbError;
    /// `NotConnected` is never retriable (it is studio-originated, not transient).
    pub fn is_retriable(&self) -> bool {
        match self {
            StudioError::NotConnected
            | StudioError::UnexpectedColumns { .. }
            | StudioError::MalformedRows { .. }
            | StudioError::MissingUsername
            | StudioError::NameTaken(_)
            | StudioError::UnknownConnection(_)
            | StudioError::UnknownDatabase(_)
            | StudioError::RevisionConflict { .. }
            | StudioError::RecordMissing { .. }
            | StudioError::PermissionDenied { .. }
            | StudioError::Rejected { .. } => false,
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
    fn a_revision_conflict_names_both_revisions_and_is_not_retriable() {
        let e = StudioError::RevisionConflict {
            collection: "users".into(),
            id: "u44182".into(),
            expected: 18,
            stored: 19,
        };
        let text = e.to_string();
        assert!(text.contains("users/u44182"), "{text}");
        assert!(
            text.contains("revision 18") && text.contains("revision 19"),
            "{text}"
        );
        assert!(!e.is_retriable());
    }

    #[test]
    fn not_connected_is_not_retriable() {
        assert!(!StudioError::NotConnected.is_retriable());
    }
}
