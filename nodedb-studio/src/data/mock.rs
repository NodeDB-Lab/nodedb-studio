//! All hardcoded mock data lives here, in one place (CLAUDE.md §10). When the
//! backend lands, the `ConnectionService` mock impl is the only thing that
//! reads this module; nothing else should hardcode data.
//!
//! Naming note: the mockup's legacy "arcadedb" labels, "local-arcade-dev"
//! name, "arcade-5" node, and per-version server tags are deliberately NOT
//! reproduced. NodeDB version numbers are undecided (CLAUDE.md §2), so the
//! server stat is a neutral "dev" placeholder rather than an invented version.

use serde::ser::{Serialize, SerializeMap, Serializer};

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
            meta: "nodedb · localhost:2480".into(),
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

// ── Streams payloads ─────────────────────────────────────────────────────────
//
// The CDC and LISTEN/NOTIFY tails show database records. Those records are
// modelled here as native typed documents (`MockDoc`), mirroring how the real
// `ConnectionService` will hand back `nodedb_types::Value` documents from the
// client. The viewers serialize them to JSON via `sonic_rs` purely for display
// — no JSON strings are carried around as data. `MockDoc` preserves field order
// (unlike `Value::Object`'s `HashMap`), so the rendered JSON is deterministic.

use FieldValue::{Float, Int, Nested, Str};

/// A scalar (or nested-document) field value inside a [`MockDoc`].
pub enum FieldValue {
    Str(&'static str),
    Int(i64),
    Float(f64),
    Nested(MockDoc),
}

/// An ordered document: `(key, value)` pairs in display order. Stands in for a
/// `nodedb_types::Value::Object` returned by the client.
pub struct MockDoc(pub Vec<(&'static str, FieldValue)>);

impl Serialize for MockDoc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in &self.0 {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

impl Serialize for FieldValue {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            FieldValue::Str(s) => serializer.serialize_str(s),
            FieldValue::Int(n) => serializer.serialize_i64(*n),
            FieldValue::Float(f) => serializer.serialize_f64(*f),
            FieldValue::Nested(doc) => doc.serialize(serializer),
        }
    }
}

fn doc(fields: Vec<(&'static str, FieldValue)>) -> MockDoc {
    MockDoc(fields)
}

/// The change operation in a CDC event.
#[derive(Clone, Copy)]
pub enum ChangeOp {
    Insert,
    Update,
    Delete,
}

impl ChangeOp {
    /// Uppercase label shown in the op column.
    pub fn label(self) -> &'static str {
        match self {
            ChangeOp::Insert => "INSERT",
            ChangeOp::Update => "UPDATE",
            ChangeOp::Delete => "DELETE",
        }
    }

    /// CSS modifier class for the op pill.
    pub fn css(self) -> &'static str {
        match self {
            ChangeOp::Insert => "ins",
            ChangeOp::Update => "upd",
            ChangeOp::Delete => "del",
        }
    }
}

/// One row in the Streams · CDC live tail.
pub struct ChangeEvent {
    pub time: &'static str,
    pub op: ChangeOp,
    pub collection: &'static str,
    pub payload: MockDoc,
    /// Optional annotation appended after the document (e.g. `⤳ +1 field`).
    pub note: Option<&'static str>,
}

/// The CDC change feed, newest first.
pub fn cdc_events() -> Vec<ChangeEvent> {
    vec![
        ChangeEvent {
            time: "04:23:18.041",
            op: ChangeOp::Insert,
            collection: "events",
            payload: doc(vec![
                ("_id", Str("evt_01HMNJ…")),
                ("type", Str("page_view")),
                ("user_id", Str("u_44182")),
                ("props", Nested(doc(vec![("path", Str("/dashboard"))]))),
            ]),
            note: None,
        },
        ChangeEvent {
            time: "04:23:18.039",
            op: ChangeOp::Update,
            collection: "sessions",
            payload: doc(vec![
                ("_id", Str("s_88209")),
                ("last_seen", Str("2026-06-13T04:23:18Z")),
            ]),
            note: Some("⤳ +1 field"),
        },
        ChangeEvent {
            time: "04:23:18.037",
            op: ChangeOp::Insert,
            collection: "events",
            payload: doc(vec![
                ("_id", Str("evt_01HMNJ…")),
                ("type", Str("click")),
                ("user_id", Str("u_77103")),
                ("props", Nested(doc(vec![("el", Str("#cta-buy"))]))),
            ]),
            note: None,
        },
        ChangeEvent {
            time: "04:23:18.035",
            op: ChangeOp::Insert,
            collection: "orders",
            payload: doc(vec![
                ("id", Int(442004)),
                ("user_id", Str("u_77103")),
                ("total", Float(89.40)),
                ("currency", Str("USD")),
            ]),
            note: None,
        },
        ChangeEvent {
            time: "04:23:18.033",
            op: ChangeOp::Delete,
            collection: "sessions_cache",
            payload: doc(vec![("key", Str("session:u_91002"))]),
            note: None,
        },
        ChangeEvent {
            time: "04:23:18.031",
            op: ChangeOp::Insert,
            collection: "events",
            payload: doc(vec![
                ("_id", Str("evt_01HMNJ…")),
                ("type", Str("scroll")),
                ("user_id", Str("u_12998")),
            ]),
            note: None,
        },
        ChangeEvent {
            time: "04:23:18.028",
            op: ChangeOp::Update,
            collection: "users",
            payload: doc(vec![
                ("_id", Str("u_44182")),
                ("last_login", Str("2026-06-13T04:23:18Z")),
            ]),
            note: None,
        },
        ChangeEvent {
            time: "04:23:18.025",
            op: ChangeOp::Insert,
            collection: "events",
            payload: doc(vec![
                ("_id", Str("evt_01HMNJ…")),
                ("type", Str("page_view")),
                ("user_id", Str("u_31001")),
                ("props", Nested(doc(vec![("path", Str("/pricing"))]))),
            ]),
            note: None,
        },
        ChangeEvent {
            time: "04:23:18.022",
            op: ChangeOp::Insert,
            collection: "events",
            payload: doc(vec![
                ("_id", Str("evt_01HMNJ…")),
                ("type", Str("form_submit")),
                ("user_id", Str("u_44182")),
                ("props", Nested(doc(vec![("form", Str("feedback"))]))),
            ]),
            note: None,
        },
    ]
}

/// A LISTEN/NOTIFY channel in the sidebar.
pub struct NotifyChannel {
    pub name: &'static str,
    pub listeners: &'static str,
    pub active: bool,
}

/// One row in the LISTEN/NOTIFY live tail.
pub struct NotifyMessage {
    pub time: &'static str,
    pub source: &'static str,
    pub payload: MockDoc,
}

/// The notify channel list.
pub fn notify_channels() -> Vec<NotifyChannel> {
    let ch = |name, listeners, active| NotifyChannel {
        name,
        listeners,
        active,
    };
    vec![
        ch("user_events", "12", true),
        ch("deploy_hooks", "3", false),
        ch("cache_invalidate", "5", false),
        ch("alerts", "8", false),
        ch("jobs_done", "14", false),
        ch("presence_room_1", "22", false),
    ]
}

/// The pub/sub message tail for the active channel.
pub fn notify_messages() -> Vec<NotifyMessage> {
    vec![
        NotifyMessage {
            time: "04:23:18.041",
            source: "api-server-2",
            payload: doc(vec![("event", Str("login")), ("user", Str("u_44182"))]),
        },
        NotifyMessage {
            time: "04:23:17.812",
            source: "webhook-relay",
            payload: doc(vec![
                ("event", Str("signup")),
                ("user", Str("u_99001")),
                ("plan", Str("pro")),
            ]),
        },
        NotifyMessage {
            time: "04:23:17.501",
            source: "api-server-1",
            payload: doc(vec![
                ("event", Str("profile_update")),
                ("user", Str("u_77103")),
            ]),
        },
        NotifyMessage {
            time: "04:23:16.998",
            source: "analytics",
            payload: doc(vec![
                ("event", Str("page_view")),
                ("user", Str("u_44182")),
                ("path", Str("/pricing")),
            ]),
        },
        NotifyMessage {
            time: "04:23:16.422",
            source: "api-server-2",
            payload: doc(vec![("event", Str("logout")), ("user", Str("u_31001"))]),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_serializes_with_fields_in_declared_order() {
        // The orders INSERT row: keys must come out in insertion order, not
        // the alphabetical/HashMap order a `Value::Object` would impose.
        let json = sonic_rs::to_string(&cdc_events()[3].payload).unwrap();
        assert_eq!(
            json,
            r#"{"id":442004,"user_id":"u_77103","total":89.4,"currency":"USD"}"#
        );
    }

    #[test]
    fn nested_document_serializes() {
        let json = sonic_rs::to_string(&cdc_events()[0].payload).unwrap();
        assert_eq!(
            json,
            r#"{"_id":"evt_01HMNJ…","type":"page_view","user_id":"u_44182","props":{"path":"/dashboard"}}"#
        );
    }
}
