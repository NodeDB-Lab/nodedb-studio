//! The active connection and its capabilities.
//!
//! Identity in NodeDB-Studio is per-connection, NOT global. There is no
//! "Studio account": switching connections swaps the NodeDB user, role, avatar
//! letter, and the capability flags that reshape the entire shell. See
//! CLAUDE.md "Per-connection identity" and "Capability-driven shell".

use serde::{Deserialize, Serialize};

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
