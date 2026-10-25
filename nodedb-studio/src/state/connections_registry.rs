//! The registry of saved connections shown on the Connection Manager and in
//! the connection switcher.
//!
//! A saved connection is not necessarily reachable. The seam answers a
//! connect for every entry. An entry without a `profile` has no known
//! identity, so its connect fails and the error bar says why.

use serde::{Deserialize, Serialize};

use crate::models::connections::{
    AuthMethod, ConnStatus, ConnectionDraft, ConnectionKind, ConnectionTarget, Observation,
    TlsConfig,
};
use crate::state::connection::{ActiveConnection, Capabilities};

/// The identity and reach a connection gets from its credentials. Everything
/// needed to build an `ActiveConnection` on connect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionProfile {
    /// The person behind the account, for the avatar and the account menu.
    pub display_name: String,
    /// The NodeDB user name, e.g. `admin`.
    pub user: String,
    /// The role this user holds, e.g. `Database administrator`.
    pub role: String,
    pub capabilities: Capabilities,
    pub databases: Vec<String>,
    pub default_database: String,
}

/// One entry in the saved-connections registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedConnection {
    pub name: String,
    pub description: String,
    pub target: ConnectionTarget,
    /// Database to open first. Empty means the profile default.
    pub database: String,
    pub auth: AuthMethod,
    pub username: String,
    pub keychain: bool,
    pub tls: TlsConfig,
    pub status: ConnStatus,
    /// Seconds since the epoch.
    pub last_used: i64,
    pub created: i64,
    /// What the last check saw. `None` before the first check.
    pub observation: Option<Observation>,
    /// Present when the identity behind the connection is known.
    pub profile: Option<ConnectionProfile>,
}

impl SavedConnection {
    pub fn kind(&self) -> ConnectionKind {
        self.target.kind()
    }

    /// Whether the credentials cannot write.
    pub fn read_only(&self) -> bool {
        self.profile
            .as_ref()
            .is_some_and(|p| p.capabilities.readonly)
    }

    /// The second line of a connection in lists: the server product with
    /// what the last check saw, or the product alone before a check.
    pub fn summary(&self) -> String {
        match (&self.observation, self.read_only()) {
            (Some(o), _) => format!("nodedb · {}ms · {} dbs", o.ping_ms, o.db_count),
            (None, true) => "nodedb · read-only".to_string(),
            (None, false) => "nodedb".to_string(),
        }
    }

    /// Build a live session from this saved entry, or `None` if it has no
    /// profile. The seam where a real client opens a socket later.
    pub fn open(&self) -> Option<ActiveConnection> {
        let profile = self.profile.as_ref()?;
        let database = if profile.databases.contains(&self.database) {
            self.database.clone()
        } else {
            profile.default_database.clone()
        };
        Some(ActiveConnection {
            name: self.name.clone(),
            sub: self.summary(),
            display_name: profile.display_name.clone(),
            user: profile.user.clone(),
            role: profile.role.clone(),
            capabilities: profile.capabilities,
            current_database: database,
        })
    }

    /// The credentials to connect with: the saved user name, else the
    /// profile's. An entry with neither yields a blank, which the seam
    /// rejects with `MissingUsername`. Never a silent fallback to `admin`.
    /// The secret is not here: it lives in the OS keychain.
    pub fn credentials(&self) -> Credentials {
        let saved = self.username.trim();
        let username = if saved.is_empty() {
            self.profile
                .as_ref()
                .map(|p| p.user.clone())
                .unwrap_or_default()
        } else {
            saved.to_string()
        };
        Credentials {
            username,
            password: None,
        }
    }

    /// Overwrite what the dialog edits. Identity, status, history, and the
    /// last observation stay: an edit changes how to connect, not what the
    /// server said last time.
    pub fn apply_draft(&mut self, draft: &ConnectionDraft) {
        self.name = draft.name.clone();
        self.target = draft.target.clone();
        self.database = draft.database.clone();
        self.auth = draft.auth;
        self.username = draft.username.clone();
        self.keychain = draft.keychain;
        self.tls = draft.tls.clone();
    }

    /// The text the account menu copies: where and as whom, never a secret.
    pub fn info_text(&self, database: &str, user: &str) -> String {
        let target = match &self.target {
            ConnectionTarget::Origin { host, port } => format!("{host}:{port}"),
            ConnectionTarget::Lite { path } => path.clone(),
        };
        let tls = match (&self.target, self.tls.required) {
            (ConnectionTarget::Origin { .. }, true) => "TLS required",
            (ConnectionTarget::Origin { .. }, false) => "TLS off",
            (ConnectionTarget::Lite { .. }, _) => "local file",
        };
        format!(
            "{} · {} · {target} · database {database} · user {user} · {tls}",
            self.name,
            self.kind().label()
        )
    }

    /// Whether the entry matches a search box: name, target, or mode,
    /// ignoring case. An empty search matches every entry.
    pub fn matches(&self, search: &str) -> bool {
        let needle = search.trim().to_lowercase();
        needle.is_empty()
            || [
                self.name.to_lowercase(),
                self.target.summary().to_lowercase(),
                self.kind().label().to_lowercase(),
            ]
            .iter()
            .any(|f| f.contains(&needle))
    }
}

/// Identity supplied at connect time. Studio never defaults the username: the
/// client would silently fall back to `admin`, so a blank field must be a
/// validation error surfaced in the connect form.
///
/// `Debug` is hand-written, not derived: a derived impl would print
/// `password` verbatim. The redaction marker is rendered unconditionally
/// (`Some` and `None` look identical) so a `{:?}` print cannot even leak
/// whether a password was set.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Credentials {
    pub username: String,
    /// A password or a token, by the connection's auth method.
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
    use crate::models::format::epoch;

    fn entry(name: &str, profile: Option<ConnectionProfile>) -> SavedConnection {
        SavedConnection {
            name: name.into(),
            description: String::new(),
            target: ConnectionTarget::Origin {
                host: "db.example".into(),
                port: 7432,
            },
            database: String::new(),
            auth: AuthMethod::UserPassword,
            username: String::new(),
            keychain: true,
            tls: TlsConfig::default(),
            status: ConnStatus::NotConnected,
            last_used: epoch(2026, 6, 1, 0, 0, 0),
            created: epoch(2026, 5, 1, 0, 0, 0),
            observation: None,
            profile,
        }
    }

    fn profile(user: &str) -> ConnectionProfile {
        ConnectionProfile {
            display_name: "Hana Idris".into(),
            user: user.into(),
            role: "Database administrator".into(),
            capabilities: Capabilities {
                graph: true,
                vector: true,
                events: true,
                timeseries: true,
                spatial: true,
                search: true,
                sync: true,
                cluster: true,
                readonly: false,
            },
            databases: vec!["analytics".into(), "commerce".into()],
            default_database: "analytics".into(),
        }
    }

    /// The seam turns a blank username into MissingUsername. A default here
    /// connects as someone the user never named.
    #[test]
    fn credentials_without_a_user_are_blank() {
        assert_eq!(entry("x", None).credentials(), Credentials::default());
    }

    #[test]
    fn saved_username_wins_over_the_profile() {
        let mut c = entry("x", Some(profile("root")));
        assert_eq!(c.credentials().username, "root");
        c.username = "  admin ".into();
        assert_eq!(c.credentials().username, "admin");
    }

    #[test]
    fn open_uses_the_saved_database_only_when_the_profile_lists_it() {
        let mut c = entry("x", Some(profile("admin")));
        c.database = "commerce".into();
        assert_eq!(
            c.open().map(|s| s.current_database).as_deref(),
            Some("commerce")
        );
        c.database = "missing".into();
        assert_eq!(
            c.open().map(|s| s.current_database).as_deref(),
            Some("analytics")
        );
        assert!(entry("y", None).open().is_none());
    }

    #[test]
    fn summary_reads_the_last_observation() {
        let mut c = entry("x", None);
        assert_eq!(c.summary(), "nodedb");
        c.observation = Some(Observation {
            at: 0,
            ping_ms: 22,
            db_count: 12,
            version: "1.4.2".into(),
            tls: crate::models::connections::TlsStatus::NotChecked,
        });
        assert_eq!(c.summary(), "nodedb · 22ms · 12 dbs");
    }

    #[test]
    fn apply_draft_round_trips_and_keeps_history() {
        let mut c = entry("x", Some(profile("admin")));
        c.last_used = 7;
        let draft = ConnectionDraft {
            name: "renamed".into(),
            target: ConnectionTarget::Lite {
                path: "/tmp/x.nodedb".into(),
            },
            database: "commerce".into(),
            auth: AuthMethod::Token,
            username: String::new(),
            keychain: false,
            tls: TlsConfig::default(),
        };
        c.apply_draft(&draft);
        assert_eq!(c.name, "renamed");
        assert_eq!(c.target, draft.target);
        assert_eq!(c.database, "commerce");
        assert_eq!(c.auth, AuthMethod::Token);
        assert!(!c.keychain);
        assert_eq!(c.last_used, 7);
        assert!(c.profile.is_some());
    }

    #[test]
    fn search_matches_name_target_and_mode() {
        let c = entry("staging-cluster", None);
        assert!(c.matches(""));
        assert!(c.matches("STAGING"));
        assert!(c.matches("db.example:7432"));
        assert!(c.matches("origin"));
        assert!(!c.matches("lite"));
    }

    /// The copied text names the place and the user. A secret has no field
    /// to come from, and the text carries none.
    #[test]
    fn info_text_carries_no_secret() {
        let mut c = entry("staging-cluster", Some(profile("admin")));
        c.tls.required = true;
        let text = c.info_text("analytics", "admin");
        assert_eq!(
            text,
            "staging-cluster · Origin · db.example:7432 · database analytics · user admin · TLS required"
        );
    }

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
