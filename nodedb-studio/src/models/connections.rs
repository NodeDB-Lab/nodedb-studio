//! Saved connections as data: where a connection points, how it
//! authenticates, what the last check observed, and the activity log.
//!
//! None of these types holds a secret. A password or token travels in
//! `Credentials` and goes to the OS keychain at the seam.

use serde::{Deserialize, Serialize};

/// What a connection opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionKind {
    /// A NodeDB Origin server or cluster, reached over the network.
    Origin,
    /// A NodeDB-Lite file on this machine.
    Lite,
}

impl ConnectionKind {
    pub fn label(self) -> &'static str {
        match self {
            ConnectionKind::Origin => "Origin",
            ConnectionKind::Lite => "Lite",
        }
    }
}

/// Where a connection points.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionTarget {
    Origin { host: String, port: u16 },
    Lite { path: String },
}

impl ConnectionTarget {
    pub fn kind(&self) -> ConnectionKind {
        match self {
            ConnectionTarget::Origin { .. } => ConnectionKind::Origin,
            ConnectionTarget::Lite { .. } => ConnectionKind::Lite,
        }
    }

    /// The Target column: `host:port`, or `Local file` for a Lite file.
    pub fn summary(&self) -> String {
        match self {
            ConnectionTarget::Origin { host, port } => format!("{host}:{port}"),
            ConnectionTarget::Lite { .. } => "Local file".to_string(),
        }
    }

    /// The host of an Origin target, for matching and for the TLS default.
    pub fn host(&self) -> Option<&str> {
        match self {
            ConnectionTarget::Origin { host, .. } => Some(host),
            ConnectionTarget::Lite { .. } => None,
        }
    }
}

/// How an Origin connection proves who the user is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthMethod {
    UserPassword,
    Token,
    SystemCredentials,
}

impl AuthMethod {
    pub const ALL: [AuthMethod; 3] = [
        AuthMethod::UserPassword,
        AuthMethod::Token,
        AuthMethod::SystemCredentials,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AuthMethod::UserPassword => "Username / password",
            AuthMethod::Token => "Token",
            AuthMethod::SystemCredentials => "Use system credentials",
        }
    }
}

/// TLS config of an Origin connection.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TlsConfig {
    pub required: bool,
    /// Name the certificate must match. Empty means the host.
    pub server_name: String,
    /// PEM bundle that signs the server certificate. Empty means the system roots.
    pub ca_file: String,
    /// Optional client certificate and key for mutual TLS.
    pub client_cert: String,
    pub client_key: String,
}

/// What the user edits in the connection dialog, minus the secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionDraft {
    pub name: String,
    pub target: ConnectionTarget,
    /// Database to open first. Empty means the server default.
    pub database: String,
    pub auth: AuthMethod,
    pub username: String,
    /// Store the password or token in the OS keychain.
    pub keychain: bool,
    pub tls: TlsConfig,
}

/// A server certificate seen by a TLS check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CertInfo {
    pub issuer: String,
    pub subject: String,
    /// Expiry, seconds since the epoch.
    pub expires_at: i64,
}

/// What a connection's TLS looked like at the last check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TlsStatus {
    /// The connection does not use TLS.
    NotUsed,
    /// No check has run yet.
    NotChecked,
    Verified(CertInfo),
}

impl TlsStatus {
    pub fn label(&self) -> &'static str {
        match self {
            TlsStatus::NotUsed => "Not used",
            TlsStatus::NotChecked => "Not checked",
            TlsStatus::Verified(_) => "Verified",
        }
    }
}

/// What the last connection check observed on the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    /// When the check ran, seconds since the epoch.
    pub at: i64,
    pub ping_ms: u32,
    pub db_count: u32,
    pub version: String,
    pub tls: TlsStatus,
}

/// Reachability at the last check, as the Status column shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnStatus {
    /// The last check reached the server.
    Available,
    /// No session and no check yet.
    NotConnected,
    /// The last check did not reach the server.
    Offline,
}

impl ConnStatus {
    pub fn label(self) -> &'static str {
        match self {
            ConnStatus::Available => "Available",
            ConnStatus::NotConnected => "Not connected",
            ConnStatus::Offline => "Offline",
        }
    }

    /// The `status-dot` modifier class.
    pub fn dot(self) -> &'static str {
        match self {
            ConnStatus::Available => "ok",
            ConnStatus::NotConnected => "idle",
            ConnStatus::Offline => "err",
        }
    }
}

/// Result of Test connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionTest {
    pub latency_ms: u32,
    pub tls_verified: bool,
    pub server_version: String,
    pub db_count: u32,
}

/// What happened in one row of the activity log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventOutcome {
    Successful,
    Failed,
    Opened,
    Disconnected,
}

impl EventOutcome {
    pub fn label(self) -> &'static str {
        match self {
            EventOutcome::Successful => "Connection successful",
            EventOutcome::Failed => "Connection failed",
            EventOutcome::Opened => "Opened",
            EventOutcome::Disconnected => "Disconnected",
        }
    }

    pub fn dot(self) -> &'static str {
        match self {
            EventOutcome::Successful | EventOutcome::Opened => "ok",
            EventOutcome::Failed => "err",
            EventOutcome::Disconnected => "idle",
        }
    }
}

/// One row of Recent activity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionEvent {
    /// Seconds since the epoch.
    pub at: i64,
    pub connection: String,
    pub kind: ConnectionKind,
    pub outcome: EventOutcome,
    pub detail: String,
}

/// The ports Origin servers listen on by default, for the dialog's first value.
pub const DEFAULT_ORIGIN_PORT: u16 = 7432;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_summary_names_the_endpoint() {
        let origin = ConnectionTarget::Origin {
            host: "db.staging.example".into(),
            port: 7432,
        };
        assert_eq!(origin.summary(), "db.staging.example:7432");
        assert_eq!(origin.kind(), ConnectionKind::Origin);
        let lite = ConnectionTarget::Lite {
            path: "/home/hana/dev.nodedb".into(),
        };
        assert_eq!(lite.summary(), "Local file");
        assert_eq!(lite.host(), None);
    }
}
