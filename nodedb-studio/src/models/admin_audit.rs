//! The audit trail: who did what to which object, and what the server said.
//!
//! An event is append-only. Studio shows what the server stored and offers
//! no edit. The trail carries permission diffs and request origins. It never
//! carries a secret or a token payload. The retention statement names the
//! permission that scopes what the reader sees: it does not promise events
//! last forever.

use crate::models::feeds::Tone;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditResult {
    Success,
    Denied,
}

impl AuditResult {
    pub const ALL: [AuditResult; 2] = [AuditResult::Success, AuditResult::Denied];

    pub fn label(self) -> &'static str {
        match self {
            AuditResult::Success => "Success",
            AuditResult::Denied => "Denied",
        }
    }

    pub fn tone(self) -> Tone {
        match self {
            AuditResult::Success => Tone::Ok,
            AuditResult::Denied => Tone::Err,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AuditRange {
    Last24Hours,
    #[default]
    Last7Days,
    Last30Days,
}

impl AuditRange {
    pub const ALL: [AuditRange; 3] = [
        AuditRange::Last24Hours,
        AuditRange::Last7Days,
        AuditRange::Last30Days,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AuditRange::Last24Hours => "Last 24 hours",
            AuditRange::Last7Days => "Last 7 days",
            AuditRange::Last30Days => "Last 30 days",
        }
    }

    pub fn seconds(self) -> i64 {
        match self {
            AuditRange::Last24Hours => 86_400,
            AuditRange::Last7Days => 7 * 86_400,
            AuditRange::Last30Days => 30 * 86_400,
        }
    }
}

/// What kind of object an event acted on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetType {
    Principal,
    Role,
    Policy,
    Collection,
    Index,
    Snapshot,
    Config,
    Credential,
}

impl TargetType {
    pub const ALL: [TargetType; 8] = [
        TargetType::Principal,
        TargetType::Role,
        TargetType::Policy,
        TargetType::Collection,
        TargetType::Index,
        TargetType::Snapshot,
        TargetType::Config,
        TargetType::Credential,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TargetType::Principal => "Principal",
            TargetType::Role => "Role",
            TargetType::Policy => "Policy",
            TargetType::Collection => "Collection",
            TargetType::Index => "Index",
            TargetType::Snapshot => "Snapshot",
            TargetType::Config => "Config",
            TargetType::Credential => "Credential",
        }
    }
}

/// Where the request came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestOrigin {
    pub ip: String,
    pub client: String,
    pub method: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEvent {
    /// Stable id, e.g. `audit104`. Ids rise with time.
    pub id: String,
    pub at: i64,
    pub actor: String,
    /// What was done, e.g. `Grant role`.
    pub action: String,
    pub target_type: TargetType,
    /// The object acted on, e.g. `user:analyst`.
    pub target: String,
    pub database: String,
    /// The database object the row shows, e.g. `analytics.sales`.
    pub object: String,
    pub result: AuditResult,
    pub correlation: String,
    /// Permissions before the action, one line each.
    pub before: Vec<String>,
    pub after: Vec<String>,
    pub origin: RequestOrigin,
}

impl AuditEvent {
    /// The principal name in a `user:name` target, so the target can link.
    pub fn principal_name(&self) -> Option<&str> {
        match self.target_type {
            TargetType::Principal => self
                .target
                .split_once(':')
                .map(|(_, name)| name)
                .or(Some(self.target.as_str())),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AuditQuery {
    pub range: AuditRange,
    pub actor: Option<String>,
    pub action: Option<String>,
    pub target: Option<TargetType>,
    pub result: Option<AuditResult>,
    /// Zero-based.
    pub page: usize,
    pub page_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditPage {
    pub events: Vec<AuditEvent>,
    /// Events matching the filters across every page.
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
    pub observed_at: i64,
}

impl AuditPage {
    pub fn pages(&self) -> usize {
        self.total.div_ceil(self.page_size.max(1)).max(1)
    }

    /// `1–10 of 47`.
    pub fn span_text(&self) -> String {
        if self.total == 0 {
            return "0 events".to_string();
        }
        let from = self.page * self.page_size + 1;
        let to = (from + self.events.len()).saturating_sub(1);
        format!("{from}–{to} of {}", self.total)
    }
}

/// What the filter menus list and what the reader may see.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditFacets {
    pub actors: Vec<String>,
    pub actions: Vec<String>,
    pub retention_days: u32,
    /// The permission that scopes the events the reader sees.
    pub scope: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(total: usize, page: usize, shown: usize) -> AuditPage {
        AuditPage {
            events: (0..shown).map(|_| blank_event()).collect(),
            total,
            page,
            page_size: 10,
            observed_at: 0,
        }
    }

    fn blank_event() -> AuditEvent {
        AuditEvent {
            id: String::new(),
            at: 0,
            actor: String::new(),
            action: String::new(),
            target_type: TargetType::Role,
            target: String::new(),
            database: String::new(),
            object: String::new(),
            result: AuditResult::Success,
            correlation: String::new(),
            before: vec![],
            after: vec![],
            origin: RequestOrigin {
                ip: String::new(),
                client: String::new(),
                method: String::new(),
            },
        }
    }

    #[test]
    fn the_span_counts_from_the_page_start() {
        assert_eq!(page(47, 0, 10).span_text(), "1–10 of 47");
        assert_eq!(page(47, 4, 7).span_text(), "41–47 of 47");
        assert_eq!(page(47, 0, 10).pages(), 5);
        assert_eq!(page(0, 0, 0).span_text(), "0 events");
        assert_eq!(page(0, 0, 0).pages(), 1);
    }

    #[test]
    fn a_principal_target_names_its_principal() {
        let mut e = blank_event();
        e.target_type = TargetType::Principal;
        e.target = "user:analyst".into();
        assert_eq!(e.principal_name(), Some("analyst"));
        e.target_type = TargetType::Role;
        assert_eq!(e.principal_name(), None);
    }
}
