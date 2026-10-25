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
    /// A write named an event or job object the database does not hold.
    #[error("no {kind} `{id}`")]
    UnknownObject { kind: &'static str, id: String },
    /// An edit named a revision of an object that has changed since. Nothing
    /// is overwritten: the editor keeps the edit and offers a reload.
    #[error(
        "{object} changed: you edited revision {expected}, the server holds revision {stored}. Reload it and make the edit again."
    )]
    StaleRevision {
        object: String,
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
    /// A save would give two saved queries one name.
    #[error("a saved query named `{0}` already exists: choose another name")]
    QueryNameTaken(String),
    /// A write named a saved query the library does not hold.
    #[error("no saved query `{0}`")]
    UnknownQuery(String),
    /// A Designer write named a staged change that is not staged.
    #[error("no staged change `{0}`")]
    UnknownChange(String),
    /// A run, explain, or console command had no statement text.
    #[error("the statement is empty")]
    EmptyStatement,
    /// The statement text did not parse.
    #[error("syntax error: {reason}")]
    Syntax { reason: String },
    /// The statement uses a parameter the request did not bind.
    #[error("parameter :{0} has no value")]
    MissingParameter(String),
    /// Commit or rollback with no transaction open.
    #[error("no transaction is open")]
    NoTransaction,
    /// Begin while a transaction is open.
    #[error("a transaction is already open: commit it or roll it back first")]
    TransactionOpen,
    /// A write inside a read-only transaction.
    #[error("the transaction is read-only: roll it back and begin a read-write one to write")]
    ReadOnlyTransaction,
}

impl StudioError {
    /// Drives the Retry affordance. Studio-originated errors are never
    /// retriable. A server error is retriable when the client says so, or
    /// when `studio_retries` names it.
    ///
    /// `true` means the condition is transient, not that re-sending is safe.
    /// The client marks `DeadlineExceeded`, `SyncConnectionFailed`,
    /// `NotLeader` and `NodeUnreachable` retriable, and each can arrive after
    /// a write committed. A Retry that re-issues a non-idempotent write must
    /// not rely on this flag alone.
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
            | StudioError::UnknownObject { .. }
            | StudioError::StaleRevision { .. }
            | StudioError::RecordMissing { .. }
            | StudioError::PermissionDenied { .. }
            | StudioError::Rejected { .. }
            | StudioError::QueryNameTaken(_)
            | StudioError::UnknownQuery(_)
            | StudioError::UnknownChange(_)
            | StudioError::EmptyStatement
            | StudioError::Syntax { .. }
            | StudioError::MissingParameter(_)
            | StudioError::NoTransaction
            | StudioError::TransactionOpen
            | StudioError::ReadOnlyTransaction => false,
            StudioError::Connection(e)
            | StudioError::Auth(e)
            | StudioError::NotFound(e)
            | StudioError::Conflict(e)
            | StudioError::ReadOnly(e)
            | StudioError::Setup(e)
            | StudioError::Server(e) => e.is_retriable() || studio_retries(e.details()),
        }
    }
}

/// Transient refusals the client does not mark retriable. Each is refused
/// before anything runs, so a later Retry click can succeed and cannot apply
/// a write twice.
fn studio_retries(details: &ErrorDetails) -> bool {
    matches!(
        details,
        // Dispatch queue full; the same request succeeds once capacity frees.
        ErrorDetails::ServerOverload
        // A rate limiter refused; the window refills.
        | ErrorDetails::RateExceeded { .. }
    )
}

/// A category constructor, such as `StudioError::Server`.
type Category = fn(NodeDbError) -> StudioError;

impl From<NodeDbError> for StudioError {
    fn from(e: NodeDbError) -> Self {
        // A variant added after nodedb-types 0.6.2 has no arm yet: it shows
        // as Server until it gets one.
        category_of(e.details()).unwrap_or(StudioError::Server)(e)
    }
}

/// The category of each `ErrorDetails` variant nodedb-types 0.6.2 defines,
/// `None` only for a variant added later. The coverage test asserts `Some`
/// for every code in `ErrorCode::ALL`, so a new variant that arrives with a
/// new code fails it. A new variant that reuses an existing code is not
/// caught: diff `details.rs` on every crate bump.
fn category_of(details: &ErrorDetails) -> Option<Category> {
    // ErrorDetails is foreign and #[non_exhaustive] -> the `_` arm is
    // REQUIRED by the compiler. This is NOT a violation of the studio's
    // "no `_ =>` on exhaustive domain enums" rule (that rule governs the
    // studio's OWN enums, e.g. StudioError above, which has no `_`).
    use ErrorDetails as D;
    let category: Category = match details {
        // Unreachable, incompatible, or refusing new work for now.
        // `QuotaExceeded` is the per-tenant or per-database connection cap,
        // raised at connect. `Cluster` is a version mismatch.
        D::HandshakeFailed { .. }
        | D::Cluster
        | D::StaleReadNotLeader { .. }
        | D::SyncConnectionFailed
        | D::NodeUnreachable
        | D::NoLeader
        | D::NotLeader { .. }
        | D::MigrationInProgress
        | D::ServerOverload
        | D::RateExceeded { .. }
        | D::QuotaExceeded { .. } => StudioError::Connection,

        D::AuthorizationDenied { .. } | D::AuthExpired | D::AuthenticationFailed => {
            StudioError::Auth
        }

        D::CollectionNotFound { .. }
        | D::DocumentNotFound { .. }
        | D::DatabaseNotFound { .. }
        | D::UndefinedObject { .. }
        | D::NotFound { .. }
        | D::CollectionDraining { .. }
        | D::CollectionDeactivated { .. } => StudioError::NotFound,

        D::WriteConflict { .. }
        | D::TransactionRollback { .. }
        | D::AlreadyExists { .. }
        | D::DependentObjectsExist { .. }
        | D::CloneDependency { .. }
        | D::MoveTenantAlreadyAtTarget { .. }
        | D::ConstraintViolation { .. }
        | D::PrevalidationRejected { .. }
        | D::BalanceViolation { .. }
        | D::StateTransitionViolation { .. }
        | D::TransitionCheckViolation { .. }
        | D::TypeGuardViolation { .. }
        | D::InsufficientBalance { .. }
        | D::Overflow { .. }
        | D::TypeMismatch { .. } => StudioError::Conflict,

        D::MirrorReadOnly { .. }
        | D::AppendOnlyViolation { .. }
        | D::PeriodLocked { .. }
        | D::LegalHoldActive { .. }
        | D::RetentionViolation { .. }
        | D::MirrorNotPromoted { .. }
        | D::CloneWriteRequiresMaterialize { .. } => StudioError::ReadOnly,

        // The request, the session, or the config must change: the same
        // request fails the same way. `ObjectNotReady` is an object in a state
        // that refuses the operation (`currval` before `nextval`, a CDC offset
        // out of range, an expired cursor). `Array` is mostly a bad array
        // operation, though an engine fault shares the variant.
        D::Config
        | D::BadRequest
        | D::SqlNotEnabled
        | D::PlanError { .. }
        | D::UndefinedColumn { .. }
        | D::AmbiguousColumn { .. }
        | D::UndefinedFunction { .. }
        | D::DivisionByZero
        | D::DataException { .. }
        | D::InvalidLimitValue { .. }
        | D::ProgramLimitExceeded { .. }
        | D::ActiveSqlTransaction { .. }
        | D::ObjectNotReady { .. }
        | D::Array { .. }
        | D::PeriodLockMisconfigured { .. }
        | D::CannotDropDefaultDatabase
        | D::QuotaOvercommit { .. }
        | D::TenantVectorDimExceeded { .. }
        | D::TenantGraphDepthExceeded { .. }
        | D::CloneDepthExceeded { .. }
        | D::CannotCloneMirror { .. }
        | D::ClonePredatesQueryTime { .. }
        | D::BackupTenantMismatch { .. }
        | D::BackupKeyMismatch
        | D::MoveTenantPreflightFailed { .. } => StudioError::Setup,

        // Server-side faults, a spent statement or memory budget, and the
        // sync-only errors Studio never triggers. `Dispatch` also covers a
        // write whose outcome is unknown (its response channel closed), so it
        // gets no Retry.
        D::DeadlineExceeded
        | D::MemoryExhausted { .. }
        | D::Dispatch { .. }
        | D::Storage { .. }
        | D::SegmentCorrupted { .. }
        | D::ColdStorage { .. }
        | D::Wal { .. }
        | D::Serialization { .. }
        | D::Codec { .. }
        | D::Encryption { .. }
        | D::Bridge { .. }
        | D::Internal { .. }
        | D::MoveTenantDrainTimeout { .. }
        | D::MoveTenantSnapshotFailed { .. }
        | D::MoveTenantCutoverFailed { .. }
        | D::SyncDeltaRejected { .. }
        | D::ShapeSubscriptionFailed { .. } => StudioError::Server,

        _ => return None,
    };
    Some(category)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodedb_types::error::ErrorCode;
    use std::mem::discriminant;

    const RETRY: bool = true;
    const FINAL: bool = false;

    /// Every code nodedb-types 0.6.2 defines, with the category and Retry flag
    /// Studio gives it. A crate bump that adds a code fails
    /// `every_error_code_has_a_deliberate_category_and_retry_flag` until the
    /// new code gets a row here and an arm in `category_of`.
    const EXPECTED: &[(ErrorCode, Category, bool)] = {
        use ErrorCode as C;
        use StudioError::{Auth, Conflict, Connection, NotFound, ReadOnly, Server, Setup};
        &[
            // Write path
            (C::CONSTRAINT_VIOLATION, Conflict, FINAL),
            (C::WRITE_CONFLICT, Conflict, RETRY),
            (C::DEADLINE_EXCEEDED, Server, RETRY),
            (C::PREVALIDATION_REJECTED, Conflict, FINAL),
            (C::APPEND_ONLY_VIOLATION, ReadOnly, FINAL),
            (C::BALANCE_VIOLATION, Conflict, FINAL),
            (C::PERIOD_LOCKED, ReadOnly, FINAL),
            (C::STATE_TRANSITION_VIOLATION, Conflict, FINAL),
            (C::TRANSITION_CHECK_VIOLATION, Conflict, FINAL),
            (C::RETENTION_VIOLATION, ReadOnly, FINAL),
            (C::LEGAL_HOLD_ACTIVE, ReadOnly, FINAL),
            (C::PERIOD_LOCK_MISCONFIGURED, Setup, FINAL),
            (C::TYPE_MISMATCH, Conflict, FINAL),
            (C::OVERFLOW, Conflict, FINAL),
            (C::INSUFFICIENT_BALANCE, Conflict, FINAL),
            (C::RATE_EXCEEDED, Connection, RETRY),
            (C::TYPE_GUARD_VIOLATION, Conflict, FINAL),
            (C::TRANSACTION_ROLLBACK, Conflict, RETRY),
            (C::ACTIVE_SQL_TRANSACTION, Setup, FINAL),
            // Read path
            (C::COLLECTION_NOT_FOUND, NotFound, FINAL),
            (C::DOCUMENT_NOT_FOUND, NotFound, FINAL),
            (C::COLLECTION_DRAINING, NotFound, FINAL),
            (C::COLLECTION_DEACTIVATED, NotFound, FINAL),
            (C::DATABASE_NOT_FOUND, NotFound, FINAL),
            (C::UNDEFINED_OBJECT, NotFound, FINAL),
            (C::ALREADY_EXISTS, Conflict, FINAL),
            (C::OBJECT_NOT_READY, Setup, FINAL),
            (C::NOT_FOUND, NotFound, FINAL),
            (C::DEPENDENT_OBJECTS_EXIST, Conflict, FINAL),
            // Query
            (C::PLAN_ERROR, Setup, FINAL),
            (C::SQL_NOT_ENABLED, Setup, FINAL),
            (C::UNDEFINED_FUNCTION, Setup, FINAL),
            (C::DIVISION_BY_ZERO, Setup, FINAL),
            (C::INVALID_LIMIT_VALUE, Setup, FINAL),
            (C::UNDEFINED_COLUMN, Setup, FINAL),
            (C::AMBIGUOUS_COLUMN, Setup, FINAL),
            (C::DATA_EXCEPTION, Setup, FINAL),
            (C::PROGRAM_LIMIT_EXCEEDED, Setup, FINAL),
            // Engine ops
            (C::ARRAY, Setup, FINAL),
            // Quota
            (C::QUOTA_OVERCOMMIT, Setup, FINAL),
            (C::TENANT_QUOTA_EXCEEDED, Connection, FINAL),
            (C::DATABASE_QUOTA_EXCEEDED, Connection, FINAL),
            (C::SERVER_OVERLOAD, Connection, RETRY),
            // Clone
            (C::CLONE_DEPTH_EXCEEDED, Setup, FINAL),
            (C::CANNOT_CLONE_MIRROR, Setup, FINAL),
            (C::CLONE_DEPENDENCY, Conflict, FINAL),
            (C::CLONE_PREDATES_QUERY_TIME, Setup, FINAL),
            (C::CLONE_WRITE_REQUIRES_MATERIALIZE, ReadOnly, FINAL),
            // Mirror
            (C::MIRROR_READ_ONLY, ReadOnly, FINAL),
            (C::STALE_READ_NOT_LEADER, Connection, FINAL),
            (C::MIRROR_NOT_PROMOTED, ReadOnly, FINAL),
            (C::CANNOT_DROP_DEFAULT_DATABASE, Setup, FINAL),
            // Move tenant
            (C::MOVE_TENANT_DRAIN_TIMEOUT, Server, FINAL),
            (C::MOVE_TENANT_PREFLIGHT_FAILED, Setup, FINAL),
            (C::MOVE_TENANT_SNAPSHOT_FAILED, Server, FINAL),
            (C::MOVE_TENANT_CUTOVER_FAILED, Server, FINAL),
            (C::MOVE_TENANT_ALREADY_AT_TARGET, Conflict, FINAL),
            // Backup and restore
            (C::BACKUP_TENANT_MISMATCH, Setup, FINAL),
            (C::BACKUP_KEY_MISMATCH, Setup, FINAL),
            // Auth and tenant limits
            (C::AUTHORIZATION_DENIED, Auth, FINAL),
            (C::AUTH_EXPIRED, Auth, FINAL),
            (C::AUTHENTICATION_FAILED, Auth, FINAL),
            (C::TENANT_VECTOR_DIM_EXCEEDED, Setup, FINAL),
            (C::TENANT_GRAPH_DEPTH_EXCEEDED, Setup, FINAL),
            // Protocol and sync
            (C::HANDSHAKE_FAILED, Connection, FINAL),
            (C::SYNC_CONNECTION_FAILED, Connection, RETRY),
            (C::SYNC_DELTA_REJECTED, Server, FINAL),
            (C::SHAPE_SUBSCRIPTION_FAILED, Server, FINAL),
            // Storage, WAL, serialization
            (C::STORAGE, Server, FINAL),
            (C::SEGMENT_CORRUPTED, Server, FINAL),
            (C::COLD_STORAGE, Server, FINAL),
            (C::WAL, Server, FINAL),
            (C::SERIALIZATION, Server, FINAL),
            (C::CODEC, Server, FINAL),
            // Config
            (C::CONFIG, Setup, FINAL),
            (C::BAD_REQUEST, Setup, FINAL),
            // Cluster and memory
            (C::NO_LEADER, Connection, RETRY),
            (C::NOT_LEADER, Connection, RETRY),
            (C::MIGRATION_IN_PROGRESS, Connection, RETRY),
            (C::NODE_UNREACHABLE, Connection, RETRY),
            (C::CLUSTER, Connection, FINAL),
            (C::MEMORY_EXHAUSTED, Server, RETRY),
            // Encryption and internal
            (C::ENCRYPTION, Server, FINAL),
            (C::INTERNAL, Server, FINAL),
            (C::BRIDGE, Server, RETRY),
            (C::DISPATCH, Server, FINAL),
        ]
    };

    /// `from_wire` builds each code's variant with placeholder fields, which
    /// is enough: the mapping reads only the variant. All mismatches are
    /// reported together, so one run lists everything a crate bump changed.
    #[test]
    fn every_error_code_has_a_deliberate_category_and_retry_flag() {
        let mut mismatches = Vec::new();
        for &code in ErrorCode::ALL {
            let rows: Vec<_> = EXPECTED.iter().filter(|(c, ..)| *c == code).collect();
            let &[&(_, category, retriable)] = rows.as_slice() else {
                mismatches.push(format!("{code}: {} rows in EXPECTED, want 1", rows.len()));
                continue;
            };
            let wire = NodeDbError::from_wire(code, "msg");
            if category_of(wire.details()).is_none() {
                mismatches.push(format!("{code}: no arm in category_of"));
                continue;
            }
            let got = StudioError::from(wire.clone());
            let want = category(wire);
            if discriminant(&got) != discriminant(&want) {
                mismatches.push(format!("{code}: maps to \"{got}\", want \"{want}\""));
            }
            if got.is_retriable() != retriable {
                mismatches.push(format!(
                    "{code}: is_retriable() is {}, want {retriable}",
                    !retriable
                ));
            }
        }
        for (code, ..) in EXPECTED {
            if !ErrorCode::ALL.contains(code) {
                mismatches.push(format!(
                    "{code}: row for a code nodedb-types no longer defines"
                ));
            }
        }
        assert!(
            mismatches.is_empty(),
            "error mapping out of date:\n{}",
            mismatches.join("\n")
        );
    }

    /// An override the client already covers is dead code. This fails when
    /// an upstream fix makes one redundant, so it can be removed.
    #[test]
    fn studio_retries_only_what_the_client_does_not() {
        for &code in ErrorCode::ALL {
            let e = NodeDbError::from_wire(code, "msg");
            assert!(
                !(studio_retries(e.details()) && e.is_retriable()),
                "{code}: the client now retries it; remove it from studio_retries"
            );
        }
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
    fn studio_originated_errors_are_not_retriable() {
        for e in [
            StudioError::NotConnected,
            StudioError::UnexpectedColumns {
                expected: "a".into(),
                got: "b".into(),
            },
            StudioError::MalformedRows { reason: "r".into() },
            StudioError::MissingUsername,
            StudioError::NameTaken("n".into()),
            StudioError::UnknownConnection("c".into()),
            StudioError::UnknownDatabase("d".into()),
            StudioError::UnknownObject {
                kind: "feed",
                id: "f".into(),
            },
            StudioError::StaleRevision {
                object: "o".into(),
                expected: 1,
                stored: 2,
            },
            StudioError::RecordMissing {
                collection: "c".into(),
                id: "i".into(),
            },
            StudioError::PermissionDenied {
                privilege: "update",
                collection: "c".into(),
            },
            StudioError::Rejected { reason: "r".into() },
            StudioError::QueryNameTaken("q".into()),
            StudioError::UnknownQuery("q".into()),
            StudioError::UnknownChange("x".into()),
            StudioError::EmptyStatement,
            StudioError::Syntax { reason: "r".into() },
            StudioError::MissingParameter("p".into()),
            StudioError::NoTransaction,
            StudioError::TransactionOpen,
            StudioError::ReadOnlyTransaction,
        ] {
            assert!(!e.is_retriable(), "{e}");
        }
    }
}
