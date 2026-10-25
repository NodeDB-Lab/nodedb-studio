//! The audit trail: ten events the design shows, forty-two older ones
//! generated from a fixed pattern, and the read that filters and pages them.

use crate::data::mock::admin::store::{AdminStore, missing};
use crate::models::admin_audit::{
    AuditEvent, AuditFacets, AuditPage, AuditQuery, AuditResult, RequestOrigin, TargetType,
};
use crate::models::format::epoch;
use crate::services::error::StudioError;

const RETENTION_DAYS: u32 = 180;

/// A correlation id: eight hex digits from a fixed mix, so a fixture and a
/// run produce the same ones.
pub(super) fn correlation(n: u32) -> String {
    let mut x = u64::from(n).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 29;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    format!("{:08x}", (x >> 16) as u32)
}

fn origin(ip: &str) -> RequestOrigin {
    RequestOrigin {
        ip: ip.to_string(),
        client: "NodeDB Studio 1.12.0".to_string(),
        method: "UI".to_string(),
    }
}

fn lines(items: &[&str]) -> Vec<String> {
    items.iter().map(|l| l.to_string()).collect()
}

struct Row {
    at: (i64, u32, u32, u32, u32, u32),
    actor: &'static str,
    action: &'static str,
    kind: TargetType,
    target: &'static str,
    object: &'static str,
    result: AuditResult,
    correlation: &'static str,
}

/// The ten events on the first page of the design, newest first.
const SHOWN: [Row; 10] = [
    Row {
        at: (2026, 6, 13, 13, 21, 0),
        actor: "admin",
        action: "Grant role",
        kind: TargetType::Principal,
        target: "user:analyst",
        object: "analytics",
        result: AuditResult::Success,
        correlation: "c5a8f2e4",
    },
    Row {
        at: (2026, 6, 13, 11, 4, 12),
        actor: "sarah",
        action: "Create user",
        kind: TargetType::Principal,
        target: "user:ml-service",
        object: "ml-service",
        result: AuditResult::Success,
        correlation: "93d2c7a1",
    },
    Row {
        at: (2026, 6, 13, 9, 18, 37),
        actor: "admin",
        action: "Update permissions",
        kind: TargetType::Collection,
        target: "collection:analytics.sales",
        object: "analytics.sales",
        result: AuditResult::Success,
        correlation: "bb41d9e8",
    },
    Row {
        at: (2026, 6, 12, 16, 42, 3),
        actor: "ci-bot",
        action: "Create index",
        kind: TargetType::Index,
        target: "index:analytics.events",
        object: "analytics.events",
        result: AuditResult::Success,
        correlation: "6f0a3c2b",
    },
    Row {
        at: (2026, 6, 12, 10, 27, 19),
        actor: "jane",
        action: "Revoke role",
        kind: TargetType::Role,
        target: "role:reporting",
        object: "reporting",
        result: AuditResult::Success,
        correlation: "d7e3b6f9",
    },
    Row {
        at: (2026, 6, 12, 9, 11, 54),
        actor: "admin",
        action: "Update user",
        kind: TargetType::Principal,
        target: "user:ml-service",
        object: "ml-service",
        result: AuditResult::Success,
        correlation: "a1f4c8d0",
    },
    Row {
        at: (2026, 6, 11, 22, 8, 31),
        actor: "ops",
        action: "Alter permissions",
        kind: TargetType::Collection,
        target: "collection:analytics.metrics",
        object: "analytics.metrics",
        result: AuditResult::Success,
        correlation: "e9b7d1a3",
    },
    Row {
        at: (2026, 6, 11, 14, 55, 6),
        actor: "sarah",
        action: "Drop index",
        kind: TargetType::Index,
        target: "index:analytics.tmp",
        object: "analytics.tmp",
        result: AuditResult::Success,
        correlation: "3c8d0f19",
    },
    Row {
        at: (2026, 6, 10, 19, 33, 44),
        actor: "admin",
        action: "Create role",
        kind: TargetType::Role,
        target: "role:data_eng",
        object: "data_eng",
        result: AuditResult::Success,
        correlation: "9a2e6b71",
    },
    Row {
        at: (2026, 6, 10, 16, 12, 10),
        actor: "tom",
        action: "Update permissions",
        kind: TargetType::Collection,
        target: "collection:analytics.customers",
        object: "analytics.customers",
        result: AuditResult::Denied,
        correlation: "5d4f0c2e",
    },
];

/// The pattern the older events follow: action, target kind, an object.
const PATTERN: [(&str, TargetType, &str); 8] = [
    (
        "Update permissions",
        TargetType::Collection,
        "analytics.orders",
    ),
    ("Create index", TargetType::Index, "analytics.articles"),
    ("Grant role", TargetType::Principal, "ingest_bot"),
    ("Update policy", TargetType::Policy, "sales_tenant_read"),
    ("Create user", TargetType::Principal, "report-viewer"),
    (
        "Update credential",
        TargetType::Credential,
        "search_embeddings",
    ),
    ("Update config", TargetType::Config, "node-2"),
    ("Drop index", TargetType::Index, "analytics.scratch"),
];

const ACTORS: [&str; 6] = ["admin", "sarah", "ci-bot", "jane", "ops", "tom"];

fn target_text(kind: TargetType, object: &str) -> String {
    let prefix = match kind {
        TargetType::Principal => "user",
        TargetType::Role => "role",
        TargetType::Policy => "policy",
        TargetType::Collection => "collection",
        TargetType::Index => "index",
        TargetType::Snapshot => "snapshot",
        TargetType::Config => "config",
        TargetType::Credential => "credential",
    };
    format!("{prefix}:{object}")
}

/// Forty-two events older than the first page, newest first. Thirty-seven
/// fall in the seven days before it and five before that.
fn older() -> Vec<AuditEvent> {
    let newest = epoch(2026, 6, 10, 15, 48, 21);
    let mut out = Vec::new();
    for i in 0..42usize {
        let id = 94 - i as u32;
        // Seven days of events 2h20m apart, then a gap before the last five.
        let at = if i < 37 {
            newest - i as i64 * 8_413
        } else {
            epoch(2026, 6, 5, 17, 2, 40) - (i as i64 - 37) * 71_113
        };
        let (action, kind, object) = PATTERN[i % PATTERN.len()];
        let actor = ACTORS[(i * 5 + 1) % ACTORS.len()];
        let denied = actor == "tom" && i % 2 == 0;
        out.push(AuditEvent {
            id: format!("audit{id}"),
            at,
            actor: actor.to_string(),
            action: action.to_string(),
            target_type: kind,
            target: target_text(kind, object),
            database: "analytics".to_string(),
            object: object.to_string(),
            result: if denied {
                AuditResult::Denied
            } else {
                AuditResult::Success
            },
            correlation: correlation(id),
            before: lines(&["State: stored"]),
            after: if denied {
                lines(&["State: stored", "The server refused the change."])
            } else {
                lines(&["State: changed"])
            },
            origin: origin(&format!("198.51.100.{}", 10 + i % 40)),
        });
    }
    out
}

pub(super) fn seeded_events() -> Vec<AuditEvent> {
    let mut out: Vec<AuditEvent> = SHOWN
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let (y, mo, d, h, mi, s) = r.at;
            let id = 104 - i as u32;
            let (before, after) = if i == 0 {
                (
                    lines(&["Roles: []", "Effective permissions: []"]),
                    lines(&[
                        "Roles: [\"analytics\"]",
                        "Effective permissions:",
                        "  - select: analytics.sales",
                        "  - select: analytics.articles",
                        "  - select: analytics.inventory (inherited)",
                    ]),
                )
            } else if r.result == AuditResult::Denied {
                (
                    lines(&["Permissions: unchanged"]),
                    lines(&["Permissions: unchanged", "The server refused the change."]),
                )
            } else {
                (lines(&["State: stored"]), lines(&["State: changed"]))
            };
            AuditEvent {
                id: format!("audit{id}"),
                at: epoch(y, mo, d, h, mi, s),
                actor: r.actor.to_string(),
                action: r.action.to_string(),
                target_type: r.kind,
                target: r.target.to_string(),
                database: "analytics".to_string(),
                object: r.object.to_string(),
                result: r.result,
                correlation: r.correlation.to_string(),
                before,
                after,
                origin: origin(if i == 0 {
                    "203.0.113.42"
                } else {
                    "198.51.100.7"
                }),
            }
        })
        .collect();
    out.extend(older());
    out
}

impl AdminStore {
    pub fn audit_facets(&self) -> AuditFacets {
        let mut actors: Vec<String> = self.events.iter().map(|e| e.actor.clone()).collect();
        actors.sort();
        actors.dedup();
        let mut actions: Vec<String> = self.events.iter().map(|e| e.action.clone()).collect();
        actions.sort();
        actions.dedup();
        AuditFacets {
            actors,
            actions,
            retention_days: RETENTION_DAYS,
            scope: "audit.read".to_string(),
        }
    }

    pub fn audit_page(&self, q: &AuditQuery) -> AuditPage {
        let floor = self.clock - q.range.seconds();
        let kept: Vec<&AuditEvent> = self
            .events
            .iter()
            .filter(|e| e.at >= floor)
            .filter(|e| q.actor.as_deref().is_none_or(|a| e.actor == a))
            .filter(|e| q.action.as_deref().is_none_or(|a| e.action == a))
            .filter(|e| q.target.is_none_or(|t| e.target_type == t))
            .filter(|e| q.result.is_none_or(|r| e.result == r))
            .collect();
        let size = q.page_size.max(1);
        let pages = kept.len().div_ceil(size).max(1);
        // A page past the end reads the last page, never an empty one.
        let page = q.page.min(pages - 1);
        AuditPage {
            events: kept
                .iter()
                .skip(page * size)
                .take(size)
                .map(|e| (*e).clone())
                .collect(),
            total: kept.len(),
            page,
            page_size: size,
            observed_at: self.clock,
        }
    }

    pub fn audit_event(&self, id: &str) -> Result<AuditEvent, StudioError> {
        self.events
            .iter()
            .find(|e| e.id == id)
            .cloned()
            .ok_or_else(|| missing("audit event", id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::admin_audit::AuditRange;

    /// The oldest id. Ids rise with time up to `audit104`.
    const FIRST_ID: u32 = 53;

    fn query(range: AuditRange) -> AuditQuery {
        AuditQuery {
            range,
            page_size: 10,
            ..AuditQuery::default()
        }
    }

    #[test]
    fn the_week_holds_47_events_and_the_first_is_audit104() {
        let s = AdminStore::seeded();
        let page = s.audit_page(&query(AuditRange::Last7Days));
        assert_eq!(page.total, 47);
        assert_eq!(page.events.len(), 10);
        assert_eq!(page.events[0].id, "audit104");
        assert_eq!(page.events[0].correlation, "c5a8f2e4");
        assert_eq!(page.span_text(), "1–10 of 47");
    }

    #[test]
    fn a_longer_range_reaches_older_events() {
        let s = AdminStore::seeded();
        assert_eq!(s.audit_page(&query(AuditRange::Last30Days)).total, 52);
        assert!(s.audit_page(&query(AuditRange::Last24Hours)).total < 10);
    }

    #[test]
    fn ids_are_unique_and_fall_with_time() {
        let s = AdminStore::seeded();
        let ids: Vec<&str> = s.events.iter().map(|e| e.id.as_str()).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len());
        assert!(
            s.events.windows(2).all(|w| w[0].at > w[1].at),
            "newest first"
        );
        assert_eq!(
            ids.last().copied(),
            Some(format!("audit{FIRST_ID}").as_str())
        );
    }

    #[test]
    fn a_page_past_the_end_reads_the_last_page() {
        let s = AdminStore::seeded();
        let mut q = query(AuditRange::Last7Days);
        q.page = 99;
        let page = s.audit_page(&q);
        assert_eq!(page.page, 4);
        assert_eq!(page.events.len(), 7);
    }

    #[test]
    fn filters_combine() {
        let s = AdminStore::seeded();
        let mut q = query(AuditRange::Last7Days);
        q.result = Some(AuditResult::Denied);
        let denied = s.audit_page(&q);
        assert!(
            denied
                .events
                .iter()
                .all(|e| e.result == AuditResult::Denied)
        );
        assert!(denied.total >= 1);
        q.actor = Some("nobody".into());
        assert_eq!(s.audit_page(&q).total, 0);
    }
}
