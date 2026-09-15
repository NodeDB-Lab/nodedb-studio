//! The active connection and its capabilities.
//!
//! Identity in NodeDB-Studio is per-connection, NOT global. There is no
//! "Studio account": switching connections swaps the NodeDB user, role, avatar
//! letter, and the capability flags that reshape the entire shell. See
//! CLAUDE.md "Per-connection identity" and "Capability-driven shell".

use serde::{Deserialize, Serialize};

use crate::services::error::StudioError;

/// A single capability flag, used both as the struct fields below and as a
/// key for the mockup's `data-cap` hide/show behavior and notification gating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Capability {
    Graph,
    Vector,
    Streams,
    Timeseries,
    Spatial,
    Fts,
    Sync,
    Cluster,
    Readonly,
}

/// What the active connection's credentials can see and do. Rail items and
/// Admin sub-tabs render conditionally on these flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    pub graph: bool,
    pub vector: bool,
    pub streams: bool,
    pub timeseries: bool,
    pub spatial: bool,
    pub fts: bool,
    /// Peer replication exists.
    pub sync: bool,
    /// Multi-node deployment.
    pub cluster: bool,
    /// Current credentials cannot write.
    pub readonly: bool,
}

impl Capabilities {
    /// Resolve a single capability flag by enum key. Lets the rail and the
    /// notification filter share one code path instead of matching fields by
    /// hand at every call site.
    pub fn has(&self, cap: Capability) -> bool {
        match cap {
            Capability::Graph => self.graph,
            Capability::Vector => self.vector,
            Capability::Streams => self.streams,
            Capability::Timeseries => self.timeseries,
            Capability::Spatial => self.spatial,
            Capability::Fts => self.fts,
            Capability::Sync => self.sync,
            Capability::Cluster => self.cluster,
            Capability::Readonly => self.readonly,
        }
    }
}

/// A live, connected NodeDB session. `None` of this exists while disconnected;
/// the app's top-level state is `Signal<Option<ActiveConnection>>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveConnection {
    pub name: String,
    /// Secondary line shown under the connection chip (e.g. "nodedb · 8.4ms · 3 dbs").
    pub sub: String,
    /// NodeDB user from this connection's credentials.
    pub user: String,
    /// Role string, e.g. "admin" or "analyst (read-only)".
    pub role: String,
    pub capabilities: Capabilities,
    pub databases: Vec<String>,
    pub current_database: String,
}

impl ActiveConnection {
    /// Avatar letter derived from the NodeDB user. Uppercased first character,
    /// `?` if the username is somehow empty.
    pub fn avatar_letter(&self) -> char {
        self.user
            .chars()
            .next()
            .map(|c| c.to_ascii_uppercase())
            .unwrap_or('?')
    }
}

/// The last failed `connect()`, surfaced app-wide.
///
/// Connect is initiated from three places, and two of them (the command palette
/// and the switch popover) close themselves the moment the attempt starts. An
/// error signal owned by those components would be dropped before it could
/// render, so the surface has to outlive them and live at the app root.
///
/// A newtype rather than a bare `Signal<Option<StudioError>>` because Dioxus
/// keys context by type: a second bare-error provider added later would bind to
/// this one instead of its own, silently.
pub struct ConnectError(pub Option<StudioError>);

/// Reconcile the active-connection slot with the result of a `connect()`.
///
/// On `Ok` the session becomes active. On `Err` the existing session is left
/// exactly as it was: a failed switch must not disconnect the user from the
/// connection they still have. The error is returned so the caller can surface
/// it rather than log it, which is the whole point — a Connect button that
/// fails silently is indistinguishable from one that is broken.
pub fn apply_connect(
    active: &mut Option<ActiveConnection>,
    result: Result<ActiveConnection, StudioError>,
) -> Option<StudioError> {
    match result {
        Ok(session) => {
            *active = Some(session);
            None
        }
        Err(e) => Some(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps() -> Capabilities {
        Capabilities {
            graph: false,
            vector: false,
            streams: false,
            timeseries: false,
            spatial: false,
            fts: false,
            sync: false,
            cluster: false,
            readonly: false,
        }
    }

    fn session(name: &str) -> ActiveConnection {
        ActiveConnection {
            name: name.into(),
            sub: "nodedb".into(),
            user: "alice".into(),
            role: "admin".into(),
            capabilities: caps(),
            databases: vec!["main".into()],
            current_database: "main".into(),
        }
    }

    #[test]
    fn apply_connect_ok_activates_the_session() {
        let mut active = None;
        let err = apply_connect(&mut active, Ok(session("local-dev")));
        assert!(err.is_none());
        assert_eq!(active.expect("session must be active").name, "local-dev");
    }

    #[test]
    fn apply_connect_err_returns_the_error_for_the_caller_to_render() {
        let mut active = None;
        let err = apply_connect(&mut active, Err(StudioError::MissingUsername));
        assert!(
            matches!(err, Some(StudioError::MissingUsername)),
            "the error must reach the caller, not be swallowed"
        );
        assert!(active.is_none());
    }

    /// A failed switch must not disconnect the user from the connection they
    /// still have. Against an implementation that clears `active` on Err, this
    /// test fails.
    #[test]
    fn apply_connect_err_keeps_the_existing_session() {
        let mut active = Some(session("local-dev"));
        let err = apply_connect(&mut active, Err(StudioError::NotConnected));
        assert!(err.is_some());
        assert_eq!(
            active
                .expect("previous session must survive a failed switch")
                .name,
            "local-dev"
        );
    }
}
