//! The active connection and its capabilities.
//!
//! Identity in NodeDB-Studio is per-connection, NOT global. There is no
//! "Studio account": switching connections swaps the person, NodeDB user,
//! role, avatar letter, and the capability flags that reshape the shell.

use serde::{Deserialize, Serialize};

use crate::services::error::StudioError;

/// A single capability flag, used both as the struct fields below and as a
/// key for the mockup's `data-cap` hide/show behavior and notification gating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Capability {
    Graph,
    Vector,
    Events,
    Timeseries,
    Spatial,
    Search,
    Sync,
    Cluster,
    Readonly,
}

/// What the active connection's credentials can see and do. Sidebar entries
/// render conditionally on these flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    pub graph: bool,
    pub vector: bool,
    /// Change feeds, topics, notifications, and scheduled jobs.
    pub events: bool,
    pub timeseries: bool,
    pub spatial: bool,
    /// Full-text and hybrid search.
    pub search: bool,
    /// Lite devices synchronize with this Origin.
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
            Capability::Events => self.events,
            Capability::Timeseries => self.timeseries,
            Capability::Spatial => self.spatial,
            Capability::Search => self.search,
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
    /// Secondary line shown under the connection chip (e.g. "nodedb · 22ms · 12 dbs").
    pub sub: String,
    /// The person behind the account, e.g. "Hana Idris".
    pub display_name: String,
    /// NodeDB user from this connection's credentials, e.g. "admin".
    pub user: String,
    /// Role this user holds, e.g. "Database administrator".
    pub role: String,
    pub capabilities: Capabilities,
    /// The database the chrome shows as selected. The list to pick from is
    /// read live through the seam's `databases()`.
    pub current_database: String,
}

impl ActiveConnection {
    /// Avatar letter: the first letter of the display name, else of the user
    /// name. Uppercased, `?` when both are empty.
    pub fn avatar_letter(&self) -> char {
        [&self.display_name, &self.user]
            .iter()
            .find_map(|s| s.trim().chars().next())
            .map(|c| c.to_uppercase().next().unwrap_or(c))
            .unwrap_or('?')
    }

    /// `admin@staging-cluster`: the user on this connection.
    pub fn account(&self) -> String {
        format!("{}@{}", self.user, self.name)
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

/// Reconcile the active session with the result of a database switch.
///
/// On `Ok` the chosen database becomes current. On `Err` the session keeps
/// the database it had: the server did not move, so the header must not say
/// it did. The error returns for the caller to show.
pub fn apply_database_switch(
    active: &mut Option<ActiveConnection>,
    database: &str,
    result: Result<(), StudioError>,
) -> Option<StudioError> {
    match result {
        Ok(()) => {
            if let Some(c) = active.as_mut() {
                c.current_database = database.to_string();
            }
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
            events: false,
            timeseries: false,
            spatial: false,
            search: false,
            sync: false,
            cluster: false,
            readonly: false,
        }
    }

    fn session(name: &str) -> ActiveConnection {
        ActiveConnection {
            name: name.into(),
            sub: "nodedb".into(),
            display_name: "Alice Ng".into(),
            user: "alice".into(),
            role: "admin".into(),
            capabilities: caps(),
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

    #[test]
    fn avatar_letter_comes_from_the_display_name() {
        let mut c = session("local-dev");
        assert_eq!(c.avatar_letter(), 'A');
        c.display_name = "hana idris".into();
        assert_eq!(c.avatar_letter(), 'H');
        c.display_name.clear();
        c.user = "root".into();
        assert_eq!(c.avatar_letter(), 'R');
        c.user.clear();
        assert_eq!(c.avatar_letter(), '?');
    }

    #[test]
    fn account_names_the_user_on_the_connection() {
        let mut c = session("staging-cluster");
        c.user = "admin".into();
        assert_eq!(c.account(), "admin@staging-cluster");
    }

    #[test]
    fn database_switch_ok_moves_the_session() {
        let mut active = Some(session("local-dev"));
        let err = apply_database_switch(&mut active, "commerce", Ok(()));
        assert!(err.is_none());
        assert_eq!(
            active.map(|c| c.current_database).as_deref(),
            Some("commerce")
        );
    }

    /// A rejected switch must leave the header on the database it named
    /// before, because the server session did not move.
    #[test]
    fn database_switch_err_keeps_the_current_database() {
        let mut active = Some(session("local-dev"));
        let err = apply_database_switch(&mut active, "commerce", Err(StudioError::NotConnected));
        assert!(err.is_some());
        assert_eq!(active.map(|c| c.current_database).as_deref(), Some("main"));
    }
}
