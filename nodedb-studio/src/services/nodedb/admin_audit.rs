//! The audit trail from `SHOW AUDIT LOG`.
//!
//! NodeDB 0.6.2 reports `seq, timestamp_us, event, tenant_id, database_id,
//! source, detail` and nothing else. It reports no actor column, no result,
//! no before/after permissions, no client name, and no correlation id. Each
//! field here is either one of those columns, read from the text of one, or
//! empty.
//!
//! - `source` is the client socket for auth events and the acting user for
//!   admin events. It is the actor only for the admin events listed in
//!   `ACTOR_IS_SOURCE`.
//! - An auth event writes `... auth: <user>` in `detail`.
//! - A `DdlChange` event writes JSON in `detail` with `descriptor_kind` and
//!   `descriptor_name`.

use std::net::SocketAddr;

use crate::models::admin_audit::{
    AuditEvent, AuditPage, AuditQuery, AuditResult, RequestOrigin, TargetType,
};
use crate::services::decode::{Table, decode_rows};
use crate::services::error::StudioError;

/// Events that record the acting user in `source`.
const ACTOR_IS_SOURCE: [&str; 9] = [
    "PrivilegeChange",
    "AdminAction",
    "DatabaseCreated",
    "DatabaseDropped",
    "DatabaseRenamed",
    "DatabaseBackedUp",
    "DatabaseRestored",
    "TenantCreated",
    "TenantDeleted",
];

/// Events the server names as a refusal.
const DENIALS: [&str; 6] = [
    "AuthFailure",
    "PermissionDenied",
    "AuthzDenied",
    "LockoutTriggered",
    "LoginRateLimited",
    "RlsRejected",
];

/// The rows `SHOW AUDIT LOG` is asked for.
// ponytail: one fetch, then filter and page in memory. When the log is
// longer than this the oldest events and the total are cut. Push the range
// into the statement when 0.6.2 can filter by time.
pub(super) const FETCH_LIMIT: usize = 5000;

fn target_type(event: &str, kind: &str, detail: &str) -> TargetType {
    let kind = kind.to_ascii_lowercase();
    let detail = detail.to_ascii_lowercase();
    if event.starts_with("Snapshot") || event.starts_with("Restore") || event.contains("BackedUp") {
        TargetType::Snapshot
    } else if kind.contains("api_key") || detail.contains("api key") {
        TargetType::Credential
    } else if kind.contains("rls") || detail.contains("rls policy") {
        TargetType::Policy
    } else if kind.contains("role") || detail.contains(" role ") {
        TargetType::Role
    } else if kind.contains("user") || event.starts_with("Auth") || event.starts_with("Session") {
        TargetType::Principal
    } else if kind.contains("index") {
        TargetType::Index
    } else if kind.contains("collection") || detail.contains("on collection") {
        TargetType::Collection
    } else {
        // ponytail: the model has no "other" kind. Config is the nearest,
        // since the server files DDL and database changes under admin actions.
        TargetType::Config
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

fn event_of(
    seq: &str,
    micros: &str,
    event: &str,
    database_id: &str,
    source: &str,
    detail: &str,
) -> Result<AuditEvent, StudioError> {
    let bad = |what: &str, v: &str| StudioError::MalformedRows {
        reason: format!("audit {what} `{v}` is not a number"),
    };
    let at = micros
        .parse::<i64>()
        .map_err(|_| bad("timestamp_us", micros))?
        / 1_000_000;
    seq.parse::<u64>().map_err(|_| bad("seq", seq))?;
    let structured = ddl(detail);
    let kind = structured.as_ref().map_or("", |d| d.kind.as_str());
    let actor = if ACTOR_IS_SOURCE.contains(&event) {
        source.to_string()
    } else if event.starts_with("Auth") {
        detail
            .rsplit_once("auth: ")
            .map(|(_, u)| u.trim().to_string())
            .unwrap_or_default()
    } else {
        String::new()
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
        target_type: target_type(event, kind, detail),
        target,
        database: if database_id == "0" {
            "default".to_string()
        } else {
            format!("database {database_id}")
        },
        object,
        result: if DENIALS.contains(&event) {
            AuditResult::Denied
        } else {
            AuditResult::Success
        },
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

/// Every event of `SHOW AUDIT LOG`, newest first.
pub(super) fn events(table: &Table) -> Result<Vec<AuditEvent>, StudioError> {
    let mut out = decode_rows(
        table,
        &[
            "seq",
            "timestamp_us",
            "event",
            "database_id",
            "source",
            "detail",
        ],
        |r| {
            event_of(
                r.field("seq")?,
                r.field("timestamp_us")?,
                r.field("event")?,
                r.field("database_id")?,
                r.field("source")?,
                r.field("detail")?,
            )
        },
    )?;
    out.sort_by_key(|e| std::cmp::Reverse(e.id.parse::<u64>().unwrap_or(0)));
    Ok(out)
}

/// The events `query` keeps and the page it asks for.
pub(super) fn page(all: Vec<AuditEvent>, query: &AuditQuery, now: i64) -> AuditPage {
    let from = now - query.range.seconds();
    let kept: Vec<AuditEvent> = all
        .into_iter()
        .filter(|e| e.at >= from)
        .filter(|e| query.actor.as_ref().is_none_or(|a| &e.actor == a))
        .filter(|e| query.action.as_ref().is_none_or(|a| &e.action == a))
        .filter(|e| query.target.is_none_or(|t| e.target_type == t))
        .filter(|e| query.result.is_none_or(|r| e.result == r))
        .collect();
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
        observed_at: now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::admin_audit::AuditRange;

    fn table(rows: &[[&str; 6]]) -> Table {
        Table {
            columns: [
                "seq",
                "timestamp_us",
                "event",
                "database_id",
                "source",
                "detail",
            ]
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
            rows: rows
                .iter()
                .map(|r| r.iter().map(|s| (*s).to_string()).collect())
                .collect(),
        }
    }

    #[test]
    fn an_auth_event_names_its_user_and_origin() {
        let t = table(&[[
            "7",
            "1795255176946394",
            "AuthSuccess",
            "0",
            "127.0.0.1:57299",
            "SCRAM-SHA-256 auth: nodedb",
        ]]);
        let e = &events(&t).expect("decodes")[0];
        assert_eq!(e.actor, "nodedb");
        assert_eq!(e.origin.ip, "127.0.0.1");
        assert_eq!(e.at, 1_795_255_176);
        assert_eq!(e.result, AuditResult::Success);
        assert_eq!(e.database, "default");
    }

    #[test]
    fn an_admin_event_takes_its_actor_from_the_source() {
        let t = table(&[[
            "8",
            "1000000",
            "PrivilegeChange",
            "0",
            "nodedb",
            "revoked SELECT on collection 'c' from 'r'",
        ]]);
        let e = &events(&t).expect("decodes")[0];
        assert_eq!(e.actor, "nodedb");
        assert_eq!(e.origin.ip, "");
        assert_eq!(e.target_type, TargetType::Collection);
    }

    #[test]
    fn a_ddl_event_reads_its_json() {
        let d = r#"{"descriptor_kind":"put_rls_policy","descriptor_name":"p1","sql_statement":"CREATE RLS POLICY p1"}"#;
        let t = table(&[["9", "1000000", "DdlChange", "0", "metadata_group", d]]);
        let e = &events(&t).expect("decodes")[0];
        assert_eq!(e.target_type, TargetType::Policy);
        assert_eq!(e.target, "p1");
        assert_eq!(e.actor, "");
    }

    #[test]
    fn a_denial_reads_as_denied() {
        let t = table(&[[
            "1",
            "1000000",
            "AuthFailure",
            "0",
            "127.0.0.1:1",
            "bad password",
        ]]);
        assert_eq!(events(&t).expect("decodes")[0].result, AuditResult::Denied);
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

    #[test]
    fn filters_and_pages_cut_the_trail() {
        let rows: Vec<[String; 6]> = (1..=25u64)
            .map(|i| {
                [
                    i.to_string(),
                    (1_000_000 * (1_000_000 + i64::try_from(i).unwrap_or(0))).to_string(),
                    if i % 2 == 0 {
                        "AuthFailure"
                    } else {
                        "AuthSuccess"
                    }
                    .to_string(),
                    "0".to_string(),
                    "127.0.0.1:1".to_string(),
                    "auth: u".to_string(),
                ]
            })
            .collect();
        let t = Table {
            columns: [
                "seq",
                "timestamp_us",
                "event",
                "database_id",
                "source",
                "detail",
            ]
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
            rows: rows.iter().map(|r| r.to_vec()).collect(),
        };
        let all = events(&t).expect("decodes");
        assert_eq!(all[0].id, "25", "newest first");
        let q = AuditQuery {
            range: AuditRange::Last24Hours,
            result: Some(AuditResult::Denied),
            page: 1,
            page_size: 5,
            ..AuditQuery::default()
        };
        let p = page(all.clone(), &q, 1_000_100);
        assert_eq!(p.total, 12);
        assert_eq!(p.events.len(), 5);
        let old = page(
            all,
            &AuditQuery {
                range: AuditRange::Last24Hours,
                page_size: 10,
                ..AuditQuery::default()
            },
            1_000_000 + 200_000,
        );
        assert_eq!(old.total, 0, "everything is older than a day");
    }
}
