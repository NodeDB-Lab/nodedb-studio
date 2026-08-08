//! The registry of saved connections shown on the Connection Manager and in
//! the connection-switch popover.
//!
//! A saved connection is not necessarily reachable. Only connections with a
//! `profile` can be connected to; an offline one (no profile) renders as a
//! disabled card, mirroring the mockup's `test-nodedb`.

use serde::{Deserialize, Serialize};

use crate::state::connection::{ActiveConnection, Capabilities};

/// Reachability/credential status as shown by the card status pill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnStatus {
    Online,
    ReadOnly,
    Offline,
}

impl ConnStatus {
    /// Whether a card with this status can be connected to at all.
    pub fn is_connectable(self) -> bool {
        !matches!(self, ConnStatus::Offline)
    }
}

/// The credential-derived details of a reachable connection — everything
/// needed to build an `ActiveConnection` on connect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionProfile {
    pub user: String,
    pub role: String,
    pub capabilities: Capabilities,
    pub databases: Vec<String>,
    pub default_database: String,
}

/// One entry in the saved-connections registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedConnection {
    pub name: String,
    /// Meta line on the card (e.g. "nodedb · single-node").
    pub meta: String,
    /// Secondary chip line once connected (e.g. "nodedb · 8.4ms · 3 dbs").
    pub sub: String,
    pub status: ConnStatus,
    /// Card stats. `None` renders as the mockup's em-dash placeholder.
    pub db_count: Option<u32>,
    pub ping: Option<String>,
    pub server: String,
    /// Present iff the connection is reachable.
    pub profile: Option<ConnectionProfile>,
}

impl SavedConnection {
    /// Build a live session from this saved entry, or `None` if it has no
    /// reachable profile (offline). The seam where a real client would
    /// actually open a socket later.
    pub fn open(&self) -> Option<ActiveConnection> {
        let profile = self.profile.as_ref()?;
        Some(ActiveConnection {
            name: self.name.clone(),
            sub: self.sub.clone(),
            user: profile.user.clone(),
            role: profile.role.clone(),
            capabilities: profile.capabilities,
            databases: profile.databases.clone(),
            current_database: profile.default_database.clone(),
        })
    }
}

/// Identity supplied at connect time. Studio never defaults the username: the
/// client would silently fall back to `admin`, so a blank field must be a
/// validation error surfaced in the connect form.
///
/// `Debug` is hand-written, not derived: a derived impl would print
/// `password` verbatim once the connect form starts populating it. The
/// redaction marker is rendered unconditionally (`Some` and `None` look
/// identical) so a `{:?}` print cannot even leak whether a password was set.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Credentials {
    pub username: String,
    pub password: Option<String>,
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_leaks_a_populated_password() {
        let creds = Credentials {
            username: "alice".into(),
            password: Some("hunter2".into()),
        };
        let printed = format!("{creds:?}");
        assert!(!printed.contains("hunter2"), "password leaked: {printed}");
        assert!(printed.contains("<redacted>"));
    }

    #[test]
    fn debug_reads_identically_whether_a_password_is_set_or_not() {
        // The redaction marker must not double as a presence/absence signal.
        let with_password = Credentials {
            username: "alice".into(),
            password: Some("hunter2".into()),
        };
        let without_password = Credentials {
            username: "alice".into(),
            password: None,
        };
        assert_eq!(
            format!("{with_password:?}"),
            format!("{without_password:?}")
        );
    }
}
