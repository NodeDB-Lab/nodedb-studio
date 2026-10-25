use crate::models::connections::{
    AuthMethod, CertInfo, ConnStatus, ConnectionDraft, ConnectionEvent, ConnectionKind,
    ConnectionTarget, EventOutcome, Observation, TlsConfig, TlsStatus,
};
use crate::models::format::epoch;
use crate::models::notification::{
    Notification, NotificationKind, NotificationTarget, ScopeKind, Severity,
};
use crate::models::shell::{NavBadges, RecentItem, RecentKind, SessionInfo};
use crate::state::connection::{Capabilities, Capability};
use crate::state::connections_registry::{ConnectionProfile, SavedConnection};

fn all_capabilities(readonly: bool) -> Capabilities {
    Capabilities {
        graph: true,
        vector: true,
        events: true,
        timeseries: true,
        spatial: true,
        search: true,
        sync: true,
        cluster: true,
        readonly,
    }
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|n| n.to_string()).collect()
}

/// The four saved connections shown on the Connection Manager. Three can be
/// connected through a profile; `test-cluster` has none and is offline.
pub fn connections() -> Vec<SavedConnection> {
    vec![
        SavedConnection {
            name: "Local development".into(),
            description: "Local Lite file".into(),
            target: ConnectionTarget::Lite {
                path: "/home/hana/nodedb/dev.nodedb".into(),
            },
            database: String::new(),
            auth: AuthMethod::SystemCredentials,
            username: String::new(),
            keychain: false,
            tls: TlsConfig::default(),
            status: ConnStatus::Available,
            last_used: epoch(2026, 6, 10, 14, 20, 0),
            created: epoch(2026, 3, 2, 9, 40, 0),
            observation: Some(Observation {
                at: epoch(2026, 6, 10, 14, 20, 33),
                ping_ms: 8,
                db_count: 3,
                version: "1.4.2".into(),
                tls: TlsStatus::NotUsed,
            }),
            profile: Some(ConnectionProfile {
                display_name: "Hana Idris".into(),
                user: "hana".into(),
                role: "Local owner".into(),
                capabilities: Capabilities {
                    sync: false,
                    cluster: false,
                    ..all_capabilities(false)
                },
                databases: names(&["analytics", "events_log", "social_graph"]),
                default_database: "analytics".into(),
            }),
        },
        SavedConnection {
            name: "staging-cluster".into(),
            description: "Staging environment".into(),
            target: ConnectionTarget::Origin {
                host: "db.staging.example".into(),
                port: 7432,
            },
            database: "analytics".into(),
            auth: AuthMethod::UserPassword,
            username: "admin".into(),
            keychain: true,
            tls: TlsConfig {
                required: true,
                server_name: "db.staging.example".into(),
                ca_file: "/etc/ssl/certs/staging-ca.pem".into(),
                ..TlsConfig::default()
            },
            status: ConnStatus::Available,
            last_used: epoch(2026, 6, 13, 13, 21, 0),
            created: epoch(2026, 5, 1, 10, 12, 0),
            observation: Some(Observation {
                at: epoch(2026, 6, 13, 13, 21, 0),
                ping_ms: 22,
                db_count: 12,
                version: "1.4.2".into(),
                tls: TlsStatus::Verified(CertInfo {
                    issuer: "R3".into(),
                    subject: "CN=db.staging.example".into(),
                    expires_at: epoch(2026, 12, 10, 13, 21, 0),
                }),
            }),
            profile: Some(ConnectionProfile {
                display_name: "Hana Idris".into(),
                user: "admin".into(),
                role: "Database administrator".into(),
                capabilities: all_capabilities(false),
                databases: staging_databases(),
                default_database: "analytics".into(),
            }),
        },
        SavedConnection {
            name: "Production".into(),
            description: "Production cluster, read-only access".into(),
            target: ConnectionTarget::Origin {
                host: "db.example.com".into(),
                port: 7432,
            },
            database: String::new(),
            auth: AuthMethod::UserPassword,
            username: "analyst".into(),
            keychain: true,
            tls: TlsConfig {
                required: true,
                ..TlsConfig::default()
            },
            status: ConnStatus::NotConnected,
            last_used: epoch(2026, 6, 10, 9, 15, 0),
            created: epoch(2026, 4, 18, 9, 2, 0),
            observation: None,
            profile: Some(ConnectionProfile {
                display_name: "Hana Idris".into(),
                user: "analyst".into(),
                role: "Read-only analyst".into(),
                capabilities: all_capabilities(true),
                databases: names(&[
                    "analytics_prod",
                    "events_prod",
                    "social_prod",
                    "orders_prod",
                    "iot_prod",
                    "docs_prod",
                    "audit_prod",
                    "cache_prod",
                ]),
                default_database: "analytics_prod".into(),
            }),
        },
        SavedConnection {
            name: "test-cluster".into(),
            description: "Throwaway test cluster".into(),
            target: ConnectionTarget::Origin {
                host: "test-db.example.com".into(),
                port: 7432,
            },
            database: String::new(),
            auth: AuthMethod::UserPassword,
            username: "admin".into(),
            keychain: true,
            tls: TlsConfig {
                required: true,
                ..TlsConfig::default()
            },
            status: ConnStatus::Offline,
            last_used: epoch(2026, 6, 8, 16, 40, 0),
            created: epoch(2026, 5, 20, 8, 0, 0),
            observation: None,
            profile: None,
        },
    ]
}

pub fn staging_databases() -> Vec<String> {
    names(&[
        "analytics",
        "commerce",
        "identity",
        "observability",
        "billing",
        "catalog",
        "inventory",
        "marketing",
        "payments",
        "reporting",
        "sessions",
        "support",
    ])
}

/// The saved entry a new connection becomes in the mock. The mock reaches
/// every host that is not a saved offline one, and signs the user in with the
/// staging profile, so a saved draft can connect.
pub fn saved_from_draft(draft: &ConnectionDraft, now: i64) -> SavedConnection {
    let staging = connections()
        .into_iter()
        .find(|c| c.name == "staging-cluster")
        .and_then(|c| c.profile);
    let profile = staging.map(|p| ConnectionProfile {
        user: if draft.username.trim().is_empty() {
            p.user.clone()
        } else {
            draft.username.trim().to_string()
        },
        ..p
    });
    SavedConnection {
        name: draft.name.clone(),
        description: String::new(),
        target: draft.target.clone(),
        database: draft.database.clone(),
        auth: draft.auth,
        username: draft.username.clone(),
        keychain: draft.keychain,
        tls: draft.tls.clone(),
        status: ConnStatus::NotConnected,
        last_used: now,
        created: now,
        observation: None,
        profile,
    }
}

/// Recent activity rows, newest first.
pub fn connection_activity() -> Vec<ConnectionEvent> {
    let event = |at, connection: &str, kind, outcome, detail: &str| ConnectionEvent {
        at,
        connection: connection.into(),
        kind,
        outcome,
        detail: detail.into(),
    };
    vec![
        event(
            epoch(2026, 6, 13, 13, 21, 0),
            "staging-cluster",
            ConnectionKind::Origin,
            EventOutcome::Successful,
            "Connected in 22ms",
        ),
        event(
            epoch(2026, 6, 12, 9, 15, 42),
            "Production",
            ConnectionKind::Origin,
            EventOutcome::Failed,
            "Connection refused",
        ),
        event(
            epoch(2026, 6, 10, 16, 40, 18),
            "test-cluster",
            ConnectionKind::Origin,
            EventOutcome::Failed,
            "Host unreachable",
        ),
        event(
            epoch(2026, 6, 10, 14, 20, 33),
            "Local development",
            ConnectionKind::Lite,
            EventOutcome::Opened,
            "Database opened (3 dbs)",
        ),
        event(
            epoch(2026, 6, 9, 11, 5, 2),
            "staging-cluster",
            ConnectionKind::Origin,
            EventOutcome::Disconnected,
            "User closed connection",
        ),
    ]
}

/// The notification feed, newest first. Capability gating is applied at
/// render time against the active connection (see `state::notifications`).
pub fn notifications() -> Vec<Notification> {
    #[allow(clippy::too_many_arguments)]
    fn entry(
        id: &str,
        kind: NotificationKind,
        severity: Severity,
        required_cap: Option<Capability>,
        title: &str,
        body: &str,
        scope: &str,
        scope_kind: ScopeKind,
        at: i64,
        target: NotificationTarget,
        unread: bool,
    ) -> Notification {
        Notification {
            id: id.into(),
            kind,
            severity,
            required_cap,
            title: title.into(),
            body: body.into(),
            scope: scope.into(),
            scope_kind,
            at,
            target,
            subject: None,
            unread,
        }
    }
    vec![
        // The Origin received mutation m104 from d17 at 13:40:00. The entry
        // opens that rejected write.
        Notification {
            subject: Some("m104".into()),
            ..entry(
                "sync-write-rejected",
                NotificationKind::SyncRejection,
                Severity::Err,
                Some(Capability::Sync),
                "Sync write rejected",
                "UNIQUE_VIOLATION for device d17.",
                "analytics.orders",
                ScopeKind::Collection,
                epoch(2026, 6, 13, 13, 40, 0),
                NotificationTarget::SyncRejectedWrites,
                true,
            )
        },
        entry(
            "index-build-embedding",
            NotificationKind::IndexBuild,
            Severity::Success,
            None,
            "Index build completed",
            "Index idx_articles_embedding has completed on collection articles.",
            "articles",
            ScopeKind::Collection,
            epoch(2026, 6, 13, 13, 21, 0),
            NotificationTarget::SearchIndexes,
            true,
        ),
        // The role grant is audit event audit104. The entry opens it.
        Notification {
            subject: Some("audit104".into()),
            ..entry(
                "role-analyst-updated",
                NotificationKind::RoleChange,
                Severity::Info,
                None,
                "Role updated",
                "Role analyst updated by admin.",
                "auth",
                ScopeKind::Role,
                epoch(2026, 6, 13, 13, 21, 0),
                NotificationTarget::AdminAudit,
                true,
            )
        },
        // The nightly snapshot of analytics finished after its 02:00 start.
        // The entry opens that snapshot.
        Notification {
            subject: Some("snap-2026-06-13-020000-4f8c2d".into()),
            ..entry(
                "backup-nightly-analytics",
                NotificationKind::Backup,
                Severity::Success,
                None,
                "Backup completed",
                "Scheduled snapshot nightly-analytics completed successfully.",
                "analytics",
                ScopeKind::Database,
                epoch(2026, 6, 13, 2, 14, 0),
                NotificationTarget::AdminBackups,
                true,
            )
        },
        entry(
            "replication-lag-normalized",
            NotificationKind::ReplicationLag,
            Severity::Info,
            Some(Capability::Cluster),
            "Replication lag normalized",
            "node-5 caught up. It was 1,309 entries behind.",
            "raft-group-1",
            ScopeKind::Cluster,
            epoch(2026, 6, 12, 21, 40, 0),
            NotificationTarget::ClusterNodes,
            false,
        ),
        entry(
            "job-vector-reindex",
            NotificationKind::JobFailure,
            Severity::Warn,
            Some(Capability::Events),
            "Scheduled job failed",
            "vector_reindex ran out of memory at step 4. The last 3 runs failed.",
            "vector_reindex",
            ScopeKind::Job,
            epoch(2026, 6, 10, 3, 0, 0),
            NotificationTarget::EventsScheduledJobs,
            false,
        ),
    ]
}

/// What the user opened lately, newest first.
pub fn recent_items() -> Vec<RecentItem> {
    vec![RecentItem {
        id: "customer-paths".into(),
        title: "Customer paths".into(),
        kind: RecentKind::GraphQuery,
    }]
}

/// Sidebar badge counts: open query tabs on the Query entry.
pub fn nav_badges() -> NavBadges {
    NavBadges { query: 3 }
}

/// The active session summary shown in the footer. `server_version` is a
/// neutral "dev" placeholder: NodeDB version numbers are undecided, so no
/// specific version is invented here.
pub fn session_info() -> SessionInfo {
    SessionInfo {
        server_version: "dev".into(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    /// Every reachable entry has a profile, or a connect would default its
    /// user name to blank and fail with a message the user cannot act on.
    #[test]
    fn every_available_entry_has_a_profile() {
        let conns = connections();
        assert!(!conns.is_empty(), "fixture must not be empty");
        for c in &conns {
            if c.status == ConnStatus::Available {
                assert!(
                    c.profile.is_some(),
                    "{} is available without a profile",
                    c.name
                );
            }
        }
    }

    #[test]
    fn connection_names_are_unique() {
        let conns = connections();
        let unique: HashSet<_> = conns.iter().map(|c| &c.name).collect();
        assert_eq!(unique.len(), conns.len());
    }

    /// The staging profile is the account the account menu shows.
    #[test]
    fn staging_matches_the_design_values() {
        let staging = connections()
            .into_iter()
            .find(|c| c.name == "staging-cluster")
            .expect("staging-cluster");
        assert_eq!(staging.summary(), "nodedb · 22ms · 12 dbs");
        let profile = staging.profile.expect("profile");
        assert_eq!(profile.databases.len(), 12);
        assert_eq!(profile.display_name, "Hana Idris");
        assert_eq!(profile.user, "admin");
        assert_eq!(profile.role, "Database administrator");
    }

    #[test]
    fn production_is_read_only_and_not_connected() {
        let prod = connections()
            .into_iter()
            .find(|c| c.name == "Production")
            .expect("Production");
        assert!(prod.read_only());
        assert_eq!(prod.status, ConnStatus::NotConnected);
    }

    #[test]
    fn notification_ids_are_unique_and_four_are_unread() {
        let feed = notifications();
        let unique: HashSet<_> = feed.iter().map(|n| &n.id).collect();
        assert_eq!(unique.len(), feed.len());
        assert_eq!(feed.iter().filter(|n| n.unread).count(), 4);
    }

    /// Every activity row names a saved connection.
    #[test]
    fn activity_names_saved_connections() {
        let saved: HashSet<_> = connections().into_iter().map(|c| c.name).collect();
        for e in connection_activity() {
            assert!(saved.contains(&e.connection), "{}", e.connection);
        }
    }
}
