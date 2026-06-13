//! All hardcoded mock data lives here, in one place (CLAUDE.md §10). When the
//! backend lands, the `ConnectionService` mock impl is the only thing that
//! reads this module; nothing else should hardcode data.
//!
//! Naming note: the mockup's legacy "arcadedb" labels, "local-arcade-dev"
//! name, "arcade-5" node, and per-version server tags are deliberately NOT
//! reproduced. NodeDB version numbers are undecided (CLAUDE.md §2), so the
//! server stat is a neutral "dev" placeholder rather than an invented version.

use crate::models::collection::{Collection, StorageMode};
use crate::models::notification::{Notification, NotificationTarget, Severity};
use crate::state::connection::{Capabilities, Capability};
use crate::state::connections_registry::{ConnStatus, ConnectionProfile, SavedConnection};

/// The four saved connections shown on the Connection Manager. Three are
/// reachable; `test-nodedb` is offline (no profile), matching the mockup.
pub fn connections() -> Vec<SavedConnection> {
    vec![
        SavedConnection {
            name: "local-nodedb-dev".into(),
            meta: "nodedb · localhost:2480".into(),
            sub: "nodedb · 8.4ms · 3 dbs".into(),
            status: ConnStatus::Online,
            db_count: Some(3),
            ping: Some("8.4ms".into()),
            server: "dev".into(),
            profile: Some(ConnectionProfile {
                user: "root".into(),
                role: "admin".into(),
                capabilities: Capabilities {
                    graph: true,
                    vector: true,
                    streams: true,
                    timeseries: true,
                    spatial: true,
                    fts: true,
                    sync: false,
                    cluster: false,
                    readonly: false,
                },
                databases: vec![
                    "analytics".into(),
                    "events_log".into(),
                    "social_graph".into(),
                ],
                default_database: "analytics".into(),
            }),
        },
        SavedConnection {
            name: "staging-cluster".into(),
            meta: "nodedb · 3-node cluster · vpn".into(),
            sub: "nodedb · 22ms · 12 dbs".into(),
            status: ConnStatus::Online,
            db_count: Some(12),
            ping: Some("22ms".into()),
            server: "dev".into(),
            profile: Some(ConnectionProfile {
                user: "hatta_admin".into(),
                role: "admin".into(),
                capabilities: Capabilities {
                    graph: true,
                    vector: true,
                    streams: true,
                    timeseries: true,
                    spatial: true,
                    fts: true,
                    sync: true,
                    cluster: true,
                    readonly: false,
                },
                databases: vec![
                    "analytics".into(),
                    "events_log".into(),
                    "social_graph".into(),
                    "iot_telemetry".into(),
                    "docs_corpus".into(),
                    "cache_layer".into(),
                    "staging_a".into(),
                    "staging_b".into(),
                    "audit_trail".into(),
                    "temp_workspace".into(),
                    "sandbox".into(),
                    "archive_2025".into(),
                ],
                default_database: "analytics".into(),
            }),
        },
        SavedConnection {
            name: "prod-replica-eu".into(),
            meta: "nodedb · eu-fra-1 · tls".into(),
            sub: "nodedb · 98ms · 28 dbs".into(),
            status: ConnStatus::ReadOnly,
            db_count: Some(28),
            ping: Some("98ms".into()),
            server: "dev".into(),
            profile: Some(ConnectionProfile {
                user: "hatta_ro".into(),
                role: "analyst (read-only)".into(),
                capabilities: Capabilities {
                    graph: true,
                    vector: true,
                    streams: true,
                    timeseries: true,
                    spatial: true,
                    fts: true,
                    sync: true,
                    cluster: true,
                    readonly: true,
                },
                databases: vec![
                    "analytics_prod".into(),
                    "events_prod".into(),
                    "social_prod".into(),
                    "orders_prod".into(),
                    "iot_prod".into(),
                    "docs_prod".into(),
                    "audit_prod".into(),
                    "cache_prod".into(),
                ],
                default_database: "analytics_prod".into(),
            }),
        },
        SavedConnection {
            name: "test-nodedb".into(),
            meta: "nodedb · localhost:5433".into(),
            sub: String::new(),
            status: ConnStatus::Offline,
            db_count: None,
            ping: None,
            server: "dev".into(),
            profile: None,
        },
    ]
}

/// Explorer collections, in sidebar display order (grouped by storage mode).
/// One NodeDB instance exposes all eight modes; these are not separate engines.
pub fn explorer_collections() -> Vec<Collection> {
    let c = |name: &str, mode, count: &str| Collection {
        name: name.to_string(),
        mode,
        count: count.to_string(),
    };
    vec![
        c("users", StorageMode::Document, "12,481"),
        c("events", StorageMode::Document, "2.4M"),
        c("sessions", StorageMode::Document, "88,209"),
        c("orders", StorageMode::Strict, "442,003"),
        c("invoices", StorageMode::Strict, "95,818"),
        c("doc_embeddings", StorageMode::Vector, "1.1M"),
        c("product_embeds", StorageMode::Vector, "88,400"),
        c("social_graph", StorageMode::Graph, "3.2M"),
        c("metrics", StorageMode::Timeseries, "48M"),
        c("sensor_temps", StorageMode::Timeseries, "5.1M"),
        c("sessions_cache", StorageMode::Kv, "18,200"),
        c("feature_flags", StorageMode::Kv, "42"),
        c("store_locations", StorageMode::Spatial, "2,108"),
        c("articles_idx", StorageMode::Fts, "241,005"),
    ]
}

/// The notification feed. Capability gating is applied at render time against
/// the active connection (see `state::notifications`).
pub fn notifications() -> Vec<Notification> {
    vec![
        Notification {
            id: "sync-conflict-1".into(),
            severity: Severity::Warn,
            group: "Sync conflicts".into(),
            required_cap: Some(Capability::Sync),
            title: "users / u_44182 / email".into(),
            desc: "Two writes within 14ms · LWW pending".into(),
            when: "2m ago".into(),
            target: NotificationTarget::Sync,
            unread: true,
        },
        Notification {
            id: "sync-conflict-2".into(),
            severity: Severity::Warn,
            group: "Sync conflicts".into(),
            required_cap: Some(Capability::Sync),
            title: "users / u_77103 / preferences".into(),
            desc: "Concurrent edit on edge-sg-1 · merge needed".into(),
            when: "4m ago".into(),
            target: NotificationTarget::Sync,
            unread: true,
        },
        Notification {
            id: "cron-fail".into(),
            severity: Severity::Err,
            group: "Scheduled jobs".into(),
            required_cap: None,
            title: "vector_reindex failed".into(),
            desc: "Out of memory at step 4 · last 3 runs failed".into(),
            when: "3d ago".into(),
            target: NotificationTarget::StreamsCron,
            unread: true,
        },
        Notification {
            id: "stream-stalled".into(),
            severity: Severity::Warn,
            group: "Streams".into(),
            required_cap: Some(Capability::Streams),
            title: "payment_failed consumer stalled".into(),
            desc: "lag 4.2s · group=fraud-svc · 1 of 2 stalled".into(),
            when: "8m ago".into(),
            target: NotificationTarget::StreamsTopics,
            unread: true,
        },
        Notification {
            id: "lag-resolved".into(),
            severity: Severity::Info,
            group: "Cluster".into(),
            required_cap: Some(Capability::Cluster),
            title: "Replication lag normalized".into(),
            desc: "node-5 caught up · was 1,309 behind".into(),
            when: "12m ago".into(),
            target: NotificationTarget::Admin,
            unread: false,
        },
        Notification {
            id: "query-done".into(),
            severity: Severity::Info,
            group: "Queries".into(),
            required_cap: None,
            title: "Long-running query finished".into(),
            desc: "\"orders_by_region_2024.sql\" · 18 rows in 2m 41s".into(),
            when: "22m ago".into(),
            target: NotificationTarget::Query,
            unread: false,
        },
    ]
}
