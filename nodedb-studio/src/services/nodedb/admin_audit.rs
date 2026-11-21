//! The audit trail.
//!
//! NodeDB 0.6.2 rows hold `seq, timestamp_us, event, tenant_id, source,
//! detail`, and `SHOW AUDIT LOG` adds `database_id`. There is no actor
//! column, no result, no before/after permissions, no client name, and no
//! correlation id. Each field here is one of those columns, read from the
//! text of one, or empty.
//!
//! Where the newest events are (server source):
//! - New events stay in memory and move to the catalog every 10 seconds.
//!   `SHOW AUDIT LOG LIMIT n` reads the catalog only: the newest `n`
//!   persisted rows, so it misses every event since the last move.
//! - `_system.audit_log` reads the catalog and the in-memory tail, and
//!   takes `WHERE`, `ORDER BY` and `LIMIT`. It has no `database_id`, and it
//!   reads at most the oldest 100,000 catalog rows before the tail.
//!
//! So Studio reads both and merges them by `seq`: `_system.audit_log` with
//! the time range and the action pushed down, and `SHOW AUDIT LOG` for the
//! database and for the newest persisted rows past 100,000. A list that may
//! miss a matching event says so (`AuditPage::truncated`).
//!
//! `KINDS` classifies every event kind the server defines: its result,
//! what it acts on, and which column names the acting user.

use std::collections::BTreeMap;
use std::net::SocketAddr;

use crate::models::admin_audit::{
    AuditEvent, AuditPage, AuditQuery, AuditResult, RequestOrigin, TargetType,
};
use crate::services::decode::{Table, decode_rows};
use crate::services::error::StudioError;

/// The most events one read keeps. Each statement asks for one more, so a
/// full read shows that more exist.
pub(super) const FETCH_LIMIT: usize = 5000;

/// The newest events, at most `FETCH_LIMIT + 1`, `$1` the oldest
/// `timestamp_us`.
pub(super) const FRESH_SQL: &str = "SELECT seq, timestamp_us, event, tenant_id, source, detail FROM _system.audit_log WHERE timestamp_us >= $1 ORDER BY seq DESC LIMIT $2";
/// `FRESH_SQL` for one action, `$3` the event name.
pub(super) const FRESH_ACTION_SQL: &str = "SELECT seq, timestamp_us, event, tenant_id, source, detail FROM _system.audit_log WHERE timestamp_us >= $1 AND event = $3 ORDER BY seq DESC LIMIT $2";
/// One event by `seq`.
pub(super) const ONE_SQL: &str = "SELECT seq, timestamp_us, event, tenant_id, source, detail FROM _system.audit_log WHERE seq = $1";

/// Where an event kind names the acting user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Actor {
    /// `source` holds the acting user or the server component that acted.
    Source,
    /// A login: `source` is the client socket, the user name is in `detail`.
    Login,
    /// No column names one.
    Nobody,
}

/// What one server event kind means. `target` is `None` when the detail
/// text tells it.
struct Kind {
    name: &'static str,
    result: AuditResult,
    target: Option<TargetType>,
    actor: Actor,
}

const fn kind(
    name: &'static str,
    result: AuditResult,
    target: Option<TargetType>,
    actor: Actor,
) -> Kind {
    Kind {
        name,
        result,
        target,
        actor,
    }
}

use Actor::{Login, Nobody, Source};
use AuditResult::{Denied, Failed, Success};
use TargetType as T;

/// Every `AuditEvent` of NodeDB 0.6.2 (`control/security/audit/event.rs`),
/// in its order. A kind the server never emits takes `Nobody`.
const KINDS: [Kind; 47] = [
    kind("AuthSuccess", Success, Some(T::Principal), Login),
    kind("AuthFailure", Denied, Some(T::Principal), Login),
    // A sync write refused: `source` is the sync session id.
    kind("AuthzDenied", Denied, Some(T::Collection), Nobody),
    kind("PrivilegeChange", Success, None, Source),
    kind("TenantCreated", Success, Some(T::Tenant), Source),
    kind("TenantDeleted", Success, Some(T::Tenant), Source),
    kind("SnapshotBegin", Success, Some(T::Snapshot), Nobody),
    kind("SnapshotEnd", Success, Some(T::Snapshot), Nobody),
    kind("RestoreBegin", Success, Some(T::Snapshot), Nobody),
    kind("RestoreEnd", Success, Some(T::Snapshot), Nobody),
    kind("CertRotation", Success, Some(T::Credential), Source),
    kind("CertRotationFailed", Failed, Some(T::Credential), Source),
    kind("KeyRotation", Success, Some(T::Credential), Nobody),
    // Emitted for collection changes: materialized sums, period locks.
    kind("ConfigChange", Success, Some(T::Collection), Source),
    kind("NodeJoined", Success, Some(T::Node), Nobody),
    kind("NodeLeft", Success, Some(T::Node), Nobody),
    kind("AdminAction", Success, None, Source),
    kind("SessionConnect", Success, Some(T::Principal), Nobody),
    kind("SessionDisconnect", Success, Some(T::Principal), Nobody),
    kind("QueryExec", Success, Some(T::Data), Nobody),
    kind("RlsDenied", Denied, Some(T::Collection), Nobody),
    kind("RowChange", Success, Some(T::Data), Nobody),
    kind("DdlChange", Success, None, Source),
    kind(
        "SessionHandleFingerprintMismatch",
        Denied,
        Some(T::Other),
        Source,
    ),
    kind(
        "SessionHandleResolveMissSpike",
        Denied,
        Some(T::Other),
        Source,
    ),
    kind("AuditCheckpoint", Success, Some(T::Other), Source),
    kind("SessionRevoked", Success, Some(T::Principal), Source),
    // Audit rows may be missing: the trail itself failed.
    kind("AuditBusLagged", Failed, Some(T::Other), Source),
    kind("PermissionDenied", Denied, None, Source),
    kind("RlsRejected", Denied, Some(T::Collection), Source),
    // `source` is the locked user.
    kind("LockoutTriggered", Denied, Some(T::Principal), Source),
    kind("LoginRateLimited", Denied, Some(T::Principal), Login),
    kind("DatabaseCreated", Success, Some(T::Database), Source),
    kind("DatabaseDropped", Success, Some(T::Database), Source),
    kind("DatabaseRenamed", Success, Some(T::Database), Source),
    kind("DatabaseQuotaChanged", Success, Some(T::Database), Source),
    kind("DatabaseCloned", Success, Some(T::Database), Source),
    kind("DatabaseMirrored", Success, Some(T::Database), Source),
    kind("DatabasePromoted", Success, Some(T::Database), Source),
    kind("DatabaseMaterialized", Success, Some(T::Database), Source),
    kind("TenantMoved", Success, Some(T::Tenant), Nobody),
    kind("DatabaseBackedUp", Success, Some(T::Snapshot), Nobody),
    kind("DatabaseRestored", Success, Some(T::Snapshot), Nobody),
    // `source` is the user id, or `unknown`.
    kind("DmlAudit", Success, Some(T::Data), Source),
    kind(
        "DatabaseAuditDmlChanged",
        Success,
        Some(T::Database),
        Source,
    ),
    kind(
        "DatabaseIdleTimeoutChanged",
        Success,
        Some(T::Database),
        Source,
    ),
    kind("OidcProviderChanged", Success, Some(T::Config), Source),
];

/// Sources that relay an event for another actor and name none.
const RELAYS: [&str; 4] = [
    "metadata_group",
    "user_change_bus",
    "session_invalidation_bus",
    "unknown",
];

/// The event kinds the action filter offers.
pub(super) fn actions() -> Vec<String> {
    KINDS.iter().map(|k| k.name.to_string()).collect()
}

/// The user a login event names in `detail`: `... auth: <user>`, `...
/// failed: <user>`, `(ip=...): <user>`, `user '<user>' ...`, or
/// `sub=<user> ...`.
fn login_user(detail: &str) -> String {
    if let Some((_, rest)) = detail.split_once("user '")
        && let Some((name, _)) = rest.split_once('\'')
    {
        return name.to_string();
    }
    let Some((_, tail)) = detail.rsplit_once(": ") else {
        return String::new();
    };
    let tail = tail
        .strip_prefix("sub=")
        .map_or(tail, |t| t.split_whitespace().next().unwrap_or_default());
    let name = tail.split(" (").next().unwrap_or_default().trim();
    if name.is_empty() || name.contains(char::is_whitespace) {
        String::new()
    } else {
        name.to_string()
    }
}

/// What a detail-driven kind acts on.
fn target_of(event: &str, kind: &str, detail: &str) -> TargetType {
    let kind = kind.to_ascii_lowercase();
    let detail = detail.to_ascii_lowercase();
    if event == "PermissionDenied" {
        return if detail.contains(" on function ") {
            TargetType::Other
        } else if detail.contains(" on tenant ") {
            TargetType::Tenant
        } else if detail.contains(" denied on '") {
            TargetType::Collection
        } else {
            TargetType::Other
        };
    }
    if kind.contains("api_key") || detail.contains("api key") {
        TargetType::Credential
    } else if kind.contains("rls") || detail.contains("rls policy") {
        TargetType::Policy
    } else if kind.contains("role") || detail.contains(" role ") {
        TargetType::Role
    } else if kind.contains("user") || detail.contains(" user ") {
        TargetType::Principal
    } else if kind.contains("index") || detail.contains(" index ") {
        TargetType::Index
    } else if kind.contains("collection") || detail.contains(" collection ") {
        TargetType::Collection
    } else if kind.contains("database") || detail.contains(" database ") {
        TargetType::Database
    } else {
        TargetType::Other
    }
}

struct Ddl {
    kind: String,
    name: String,
    sql: String,
}

fn ddl(detail: &str) -> Option<Ddl> {
    use sonic_rs::JsonValueTrait;
    let v: sonic_rs::Value = sonic_rs::from_str(detail).ok()?;
    let text = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
    Some(Ddl {
        kind: text("descriptor_kind")?,
        name: text("descriptor_name").unwrap_or_default(),
        sql: text("sql_statement").unwrap_or_default(),
    })
}

/// The label of a `database_id`. The server writes `0` both for the default
/// database and for an event with no database.
fn database_label(id: &str) -> String {
    if id == "0" {
        "default or none".to_string()
    } else {
        format!("database {id}")
    }
}

fn event_of(
    micros: &str,
    seq: u64,
    event: &str,
    database_id: Option<&str>,
    source: &str,
    detail: &str,
) -> Result<AuditEvent, StudioError> {
    let at = micros
        .parse::<i64>()
        .map_err(|_| StudioError::MalformedRows {
            reason: format!("audit timestamp_us `{micros}` is not a number"),
        })?
        / 1_000_000;
    let known = KINDS.iter().find(|k| k.name == event);
    let structured = ddl(detail);
    let ddl_kind = structured.as_ref().map_or("", |d| d.kind.as_str());
    let actor = match known.map_or(Nobody, |k| k.actor) {
        Source if RELAYS.contains(&source) => String::new(),
        Source => source.to_string(),
        Login => login_user(detail),
        Nobody => String::new(),
    };
    let result = match known {
        None => AuditResult::Unknown,
        Some(_) if source == "event_trigger_failed" => AuditResult::Failed,
        Some(k) => k.result,
    };
    let target_type = match known {
        None => TargetType::Other,
        Some(k) => k
            .target
            .unwrap_or_else(|| target_of(event, ddl_kind, detail)),
    };
    let (target, object) = match &structured {
        Some(d) => (d.name.clone(), d.sql.clone()),
        None => (String::new(), detail.to_string()),
    };
    Ok(AuditEvent {
        id: seq.to_string(),
        at,
        actor,
        action: event.to_string(),
        target_type,
        target,
        database: database_id.map(database_label),
        object,
        result,
        correlation: String::new(),
        before: vec![],
        after: vec![],
        origin: RequestOrigin {
            ip: source
                .parse::<SocketAddr>()
                .map(|a| a.ip().to_string())
                .unwrap_or_default(),
            client: String::new(),
            method: String::new(),
        },
    })
}

/// The events of `SHOW AUDIT LOG` or `_system.audit_log`, keyed by `seq`.
/// The second has no `database_id`, so its events report no database.
pub(super) fn events(table: &Table) -> Result<Vec<(u64, AuditEvent)>, StudioError> {
    let with_db = table.columns.iter().any(|c| c == "database_id");
    decode_rows(
        table,
        &["seq", "timestamp_us", "event", "source", "detail"],
        |r| {
            let seq = r.field("seq")?;
            let seq = seq.parse::<u64>().map_err(|_| StudioError::MalformedRows {
                reason: format!("audit seq `{seq}` is not a number"),
            })?;
            let database_id = if with_db {
                Some(r.field("database_id")?)
            } else {
                None
            };
            Ok((
                seq,
                event_of(
                    r.field("timestamp_us")?,
                    seq,
                    r.field("event")?,
                    database_id,
                    r.field("source")?,
                    r.field("detail")?,
                )?,
            ))
        },
    )
}

/// The two reads of one page. `fresh`: `_system.audit_log` with the range
/// and action pushed down. `stored`: `SHOW AUDIT LOG`. Each holds at most
/// `FETCH_LIMIT + 1` rows. `gap`: the catalog holds more rows than
/// `_system.audit_log` reads, so `fresh` skips the newest persisted ones.
pub(super) struct Fetched {
    pub fresh: Vec<(u64, AuditEvent)>,
    pub stored: Vec<(u64, AuditEvent)>,
    pub gap: bool,
}

/// The events `query` keeps, newest first, and the page it asks for.
pub(super) fn page(f: Fetched, query: &AuditQuery, now: i64) -> AuditPage {
    let from = now - query.range.seconds();
    let stored_full = f.stored.len() > FETCH_LIMIT;
    // Below the oldest stored row, `fresh` has a hole when there is a gap.
    let floor = f
        .stored
        .last()
        .filter(|_| f.gap && stored_full)
        .map(|(seq, e)| (*seq, e.at));
    let mut truncated = f.fresh.len() > FETCH_LIMIT || floor.is_some_and(|(_, at)| at >= from);
    let mut by_seq: BTreeMap<u64, AuditEvent> = f
        .fresh
        .into_iter()
        .filter(|(seq, _)| floor.is_none_or(|(low, _)| *seq >= low))
        .collect();
    // A stored row carries the database: it replaces the fresh copy.
    by_seq.extend(f.stored);
    let mut kept: Vec<AuditEvent> = by_seq
        .into_values()
        .rev()
        .filter(|e| e.at >= from)
        .filter(|e| query.actor.as_ref().is_none_or(|a| &e.actor == a))
        .filter(|e| query.action.as_ref().is_none_or(|a| &e.action == a))
        .filter(|e| query.target.is_none_or(|t| e.target_type == t))
        .filter(|e| query.result.is_none_or(|r| e.result == r))
        .collect();
    if kept.len() > FETCH_LIMIT {
        kept.truncate(FETCH_LIMIT);
        truncated = true;
    }
    let size = query.page_size.max(1);
    AuditPage {
        total: kept.len(),
        events: kept
            .into_iter()
            .skip(query.page.saturating_mul(size))
            .take(size)
            .collect(),
        page: query.page,
        page_size: size,
        truncated,
        observed_at: now,
    }
}

/// Event `seq` from either read. The stored copy names its database.
pub(super) fn find(
    fresh: Vec<(u64, AuditEvent)>,
    stored: Vec<(u64, AuditEvent)>,
    seq: u64,
) -> Option<AuditEvent> {
    stored
        .into_iter()
        .chain(fresh)
        .find(|(s, _)| *s == seq)
        .map(|(_, e)| e)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::admin_audit::AuditRange;

    const COLS: [&str; 7] = [
        "seq",
        "timestamp_us",
        "event",
        "tenant_id",
        "database_id",
        "source",
        "detail",
    ];

    /// `SHOW AUDIT LOG` rows: `seq, timestamp_us, event, database_id,
    /// source, detail`.
    fn table(rows: &[[&str; 6]]) -> Table {
        Table {
            columns: COLS.iter().map(|s| (*s).to_string()).collect(),
            rows: rows
                .iter()
                .map(|r| {
                    let mut v: Vec<String> = r.iter().map(|s| (*s).to_string()).collect();
                    v.insert(3, "0".to_string());
                    v
                })
                .collect(),
        }
    }

    fn one(row: [&str; 6]) -> AuditEvent {
        events(&table(&[row])).expect("decodes").remove(0).1
    }

    /// Every `AuditEvent` variant of the NodeDB 0.6.2 server.
    const SERVER_KINDS: [&str; 47] = [
        "AuthSuccess",
        "AuthFailure",
        "AuthzDenied",
        "PrivilegeChange",
        "TenantCreated",
        "TenantDeleted",
        "SnapshotBegin",
        "SnapshotEnd",
        "RestoreBegin",
        "RestoreEnd",
        "CertRotation",
        "CertRotationFailed",
        "KeyRotation",
        "ConfigChange",
        "NodeJoined",
        "NodeLeft",
        "AdminAction",
        "SessionConnect",
        "SessionDisconnect",
        "QueryExec",
        "RlsDenied",
        "RowChange",
        "DdlChange",
        "SessionHandleFingerprintMismatch",
        "SessionHandleResolveMissSpike",
        "AuditCheckpoint",
        "SessionRevoked",
        "AuditBusLagged",
        "PermissionDenied",
        "RlsRejected",
        "LockoutTriggered",
        "LoginRateLimited",
        "DatabaseCreated",
        "DatabaseDropped",
        "DatabaseRenamed",
        "DatabaseQuotaChanged",
        "DatabaseCloned",
        "DatabaseMirrored",
        "DatabasePromoted",
        "DatabaseMaterialized",
        "TenantMoved",
        "DatabaseBackedUp",
        "DatabaseRestored",
        "DmlAudit",
        "DatabaseAuditDmlChanged",
        "DatabaseIdleTimeoutChanged",
        "OidcProviderChanged",
    ];

    #[test]
    fn every_server_event_kind_is_classified_and_no_other() {
        let ours: Vec<&str> = KINDS.iter().map(|k| k.name).collect();
        for k in SERVER_KINDS {
            assert!(
                ours.contains(&k),
                "`{k}` has no result, target or actor rule"
            );
        }
        assert_eq!(
            ours, SERVER_KINDS,
            "one rule per kind, in the server's order"
        );
    }

    #[test]
    fn an_unknown_kind_is_never_a_success() {
        let e = one(["1", "1000000", "Teleported", "0", "nodedb", "x"]);
        assert_eq!(e.result, AuditResult::Unknown);
        assert_eq!(e.target_type, TargetType::Other);
        assert_eq!(e.actor, "", "no rule, no actor");
    }

    #[test]
    fn denials_and_failures_read_as_such() {
        for (kind, result) in [
            ("AuthFailure", AuditResult::Denied),
            ("RlsDenied", AuditResult::Denied),
            ("RlsRejected", AuditResult::Denied),
            ("SessionHandleFingerprintMismatch", AuditResult::Denied),
            ("SessionHandleResolveMissSpike", AuditResult::Denied),
            ("CertRotationFailed", AuditResult::Failed),
            ("AuditBusLagged", AuditResult::Failed),
            ("DdlChange", AuditResult::Success),
        ] {
            assert_eq!(
                one(["1", "1000000", kind, "0", "s", "d"]).result,
                result,
                "{kind}"
            );
        }
        let failed = one([
            "1",
            "1000000",
            "AdminAction",
            "0",
            "event_trigger_failed",
            "t",
        ]);
        assert_eq!(failed.result, AuditResult::Failed);
    }

    #[test]
    fn a_query_or_row_event_is_data_not_config() {
        for (kind, target) in [
            ("QueryExec", TargetType::Data),
            ("RowChange", TargetType::Data),
            ("DmlAudit", TargetType::Data),
            ("NodeJoined", TargetType::Node),
            ("KeyRotation", TargetType::Credential),
            ("CertRotation", TargetType::Credential),
            ("TenantMoved", TargetType::Tenant),
            ("DatabaseRenamed", TargetType::Database),
            ("AuditCheckpoint", TargetType::Other),
            ("AdminAction", TargetType::Other),
        ] {
            assert_eq!(
                one(["1", "1000000", kind, "0", "nodedb", "CREATE FUNCTION f"]).target_type,
                target,
                "{kind}"
            );
        }
    }

    #[test]
    fn a_denied_permission_names_the_denied_user_as_actor() {
        let e = one([
            "5",
            "1000000",
            "PermissionDenied",
            "0",
            "ann",
            "permission Drop denied on 'orders' for user 'ann'",
        ]);
        assert_eq!(e.actor, "ann");
        assert_eq!(e.target_type, TargetType::Collection);
        let q = AuditQuery {
            range: AuditRange::Last24Hours,
            actor: Some("ann".into()),
            page_size: 10,
            ..AuditQuery::default()
        };
        let p = page(
            Fetched {
                fresh: vec![(5, e)],
                stored: vec![],
                gap: false,
            },
            &q,
            1,
        );
        assert_eq!(p.total, 1, "the actor filter keeps the denial");
    }

    #[test]
    fn a_login_names_its_user_wherever_the_detail_puts_it() {
        for (detail, user) in [
            ("SCRAM-SHA-256 auth: nodedb", "nodedb"),
            ("native password auth failed: ann", "ann"),
            ("native api_key auth: bot (key 7)", "bot"),
            ("user 'ann' is locked out", "ann"),
            ("login rate limited (ip=10.0.0.1): ann", "ann"),
            ("OIDC bearer login: sub=ann method=oidc_bearer", "ann"),
            ("trust auth rejected: server requires authentication", ""),
            ("mTLS auth failed: no user for cert CN 'x'", ""),
        ] {
            assert_eq!(login_user(detail), user, "{detail}");
        }
        let e = one([
            "7",
            "1795255176946394",
            "AuthSuccess",
            "0",
            "127.0.0.1:57299",
            "SCRAM-SHA-256 auth: nodedb",
        ]);
        assert_eq!(e.actor, "nodedb");
        assert_eq!(e.origin.ip, "127.0.0.1");
        assert_eq!(e.at, 1_795_255_176);
        assert_eq!(e.database.as_deref(), Some("default or none"));
    }

    #[test]
    fn an_admin_event_takes_its_actor_from_the_source() {
        let e = one([
            "8",
            "1000000",
            "PrivilegeChange",
            "1024",
            "nodedb",
            "revoked SELECT on collection 'c' from 'r'",
        ]);
        assert_eq!(e.actor, "nodedb");
        assert_eq!(e.origin.ip, "");
        assert_eq!(e.target_type, TargetType::Collection);
        assert_eq!(e.database.as_deref(), Some("database 1024"));
    }

    #[test]
    fn a_ddl_event_reads_its_json_and_a_relay_is_no_actor() {
        let d = r#"{"descriptor_kind":"put_rls_policy","descriptor_name":"p1","sql_statement":"CREATE RLS POLICY p1"}"#;
        let e = one(["9", "1000000", "DdlChange", "0", "metadata_group", d]);
        assert_eq!(e.target_type, TargetType::Policy);
        assert_eq!(e.target, "p1");
        assert_eq!(e.actor, "");
    }

    #[test]
    fn a_bad_number_is_an_error() {
        let t = table(&[["x", "1", "AuthSuccess", "0", "s", "d"]]);
        assert!(events(&t).is_err());
        assert!(
            events(&Table {
                columns: vec!["setting".into()],
                rows: vec![vec![String::new()]]
            })
            .is_err()
        );
    }

    /// `n` login events, seq `first..`, one second apart from `at`.
    fn run(first: u64, n: u64, at: i64, with_db: bool) -> Vec<(u64, AuditEvent)> {
        (first..first + n)
            .map(|i| {
                let t = i64::try_from(i).unwrap_or(0) + at;
                let e = event_of(
                    &(t * 1_000_000).to_string(),
                    i,
                    if i % 2 == 0 {
                        "AuthFailure"
                    } else {
                        "AuthSuccess"
                    },
                    with_db.then_some("0"),
                    "127.0.0.1:1",
                    "auth: u",
                )
                .expect("event");
                (i, e)
            })
            .collect()
    }

    fn query(range: AuditRange) -> AuditQuery {
        AuditQuery {
            range,
            page_size: 10,
            ..AuditQuery::default()
        }
    }

    #[test]
    fn the_unflushed_tail_shows_first_and_a_stored_copy_names_its_database() {
        // Seq 1-20 are in the catalog, 21-25 only in memory.
        let p = page(
            Fetched {
                fresh: run(1, 25, 1_000_000, false).into_iter().rev().collect(),
                stored: run(1, 20, 1_000_000, true).into_iter().rev().collect(),
                gap: false,
            },
            &query(AuditRange::Last24Hours),
            1_000_100,
        );
        assert_eq!(p.events[0].id, "25", "newest first");
        assert_eq!(p.events[0].database, None, "not flushed yet");
        assert_eq!(p.events[5].id, "20");
        assert_eq!(p.events[5].database.as_deref(), Some("default or none"));
        assert_eq!(p.total, 25);
        assert!(!p.truncated);
    }

    #[test]
    fn a_full_read_says_the_list_is_cut() {
        let n = u64::try_from(FETCH_LIMIT).unwrap_or(0) + 1;
        let p = page(
            Fetched {
                fresh: run(1, n, 1_000_000, false).into_iter().rev().collect(),
                stored: vec![],
                gap: false,
            },
            &query(AuditRange::Last30Days),
            1_010_000,
        );
        assert!(p.truncated);
        assert_eq!(p.total, FETCH_LIMIT);
        assert_eq!(p.events[0].id, n.to_string(), "the newest are kept");
    }

    #[test]
    fn past_the_catalog_read_limit_the_list_stops_at_the_stored_rows() {
        let n = u64::try_from(FETCH_LIMIT).unwrap_or(0) + 1;
        // `_system.audit_log` reads seq 1-10 of the catalog and the tail
        // 300000-300004. The catalog's newest rows are seq 200000 up.
        let mut fresh = run(300_000, 5, 1_000_000, false);
        fresh.extend(run(1, 10, 0, false));
        fresh.reverse();
        let stored: Vec<_> = run(200_000, n, 800_000, true).into_iter().rev().collect();
        let p = page(
            Fetched {
                fresh,
                stored,
                gap: true,
            },
            &query(AuditRange::Last30Days),
            1_310_000,
        );
        assert!(p.truncated, "older events in range are not read");
        assert!(
            p.events
                .iter()
                .all(|e| e.id.parse::<u64>().unwrap_or(0) >= 200_000)
        );
        assert_eq!(p.events[0].id, "300004");
    }

    #[test]
    fn filters_and_pages_cut_the_trail() {
        let all = run(1, 25, 1_000_000, true)
            .into_iter()
            .rev()
            .collect::<Vec<_>>();
        let q = AuditQuery {
            range: AuditRange::Last24Hours,
            result: Some(AuditResult::Denied),
            page: 1,
            page_size: 5,
            ..AuditQuery::default()
        };
        let fetched = |v: &Vec<(u64, AuditEvent)>| Fetched {
            fresh: v.clone(),
            stored: vec![],
            gap: false,
        };
        let p = page(fetched(&all), &q, 1_000_100);
        assert_eq!(p.total, 12);
        assert_eq!(p.events.len(), 5);
        let old = page(fetched(&all), &query(AuditRange::Last24Hours), 1_200_000);
        assert_eq!(old.total, 0, "everything is older than a day");
    }

    #[test]
    fn one_event_prefers_the_stored_copy() {
        let fresh = run(3, 1, 0, false);
        let stored = run(3, 1, 0, true);
        let e = find(fresh.clone(), stored, 3).expect("found");
        assert!(e.database.is_some());
        assert!(find(fresh, vec![], 4).is_none());
    }
}
