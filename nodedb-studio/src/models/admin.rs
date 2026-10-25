//! What every Administration change shares: the review a person reads
//! before anything applies, and the one enum that names every change.
//!
//! A change is previewed first. The preview carries the revision of the
//! objects it read, and apply names that revision. The server refuses an
//! apply when the objects changed in between, so a stale review never
//! overwrites another administrator's work.

use crate::models::admin_access::AccessChange;
use crate::models::admin_backup::BackupChange;
use crate::models::admin_config::ConfigRequest;
use crate::models::admin_credential::CredentialChange;
use crate::models::admin_policy::PolicyChange;
use crate::models::cluster_op::PermissionCheck;

/// One line of a review: what changes, from what, to what.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewLine {
    pub label: String,
    /// The stored value. `None` for something that does not exist yet.
    pub before: Option<String>,
    pub after: String,
}

impl ReviewLine {
    pub fn new(label: impl Into<String>, before: Option<&str>, after: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            before: before.map(str::to_string),
            after: after.into(),
        }
    }
}

/// What a change would do, read without doing it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminReview {
    pub title: String,
    /// The object the change names, for the sheet's first line.
    pub target: String,
    pub lines: Vec<ReviewLine>,
    /// Facts to weigh before applying.
    pub notes: Vec<String>,
    /// Risks the person must read.
    pub warnings: Vec<String>,
    /// Reasons the change cannot apply. Any one blocks it.
    pub errors: Vec<String>,
    pub permission: PermissionCheck,
    /// The revision the preview read. Apply names it.
    pub revision: u64,
    /// The label of the button that applies it.
    pub apply_label: String,
}

impl AdminReview {
    pub fn can_apply(&self) -> bool {
        self.permission.granted && self.errors.is_empty() && !self.lines.is_empty()
    }
}

/// Every change Administration applies.
#[derive(Debug, Clone, PartialEq)]
pub enum AdminChange {
    Access(Vec<AccessChange>),
    Policy(PolicyChange),
    Backup(BackupChange),
    Config(ConfigRequest),
    Credential(CredentialChange),
}

/// A name for a principal, role, policy, or database: lowercase letters,
/// digits, and underscores, starting with a letter, up to 63 characters.
pub fn identifier_issue(name: &str, what: &str) -> Option<String> {
    let name = name.trim();
    let mut chars = name.chars();
    let first = chars.next();
    if first.is_none() {
        Some(format!("Enter a {what} name."))
    } else if name.len() > 63 {
        Some("Use 63 characters or fewer.".to_string())
    } else if !first.is_some_and(|c| c.is_ascii_lowercase())
        || !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        Some(
            "Start with a lowercase letter. Use lowercase letters, digits, and underscores."
                .to_string(),
        )
    } else {
        None
    }
}

/// `2026-12-31` as seconds since the epoch at 00:00 UTC.
pub fn parse_date(text: &str) -> Option<i64> {
    let mut parts = text.trim().split('-');
    let y: i64 = parts.next()?.parse().ok()?;
    let m: u32 = parts.next()?.parse().ok()?;
    let d: u32 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let at = crate::models::format::epoch(y, m, d, 0, 0, 0);
    // A day that rolls over (31 February) does not round-trip.
    (crate::models::format::date(at, 0) == format!("{y:04}-{m:02}-{d:02}")).then_some(at)
}

/// `18 GiB`, `512 MiB`, `0 B`.
pub fn bytes_text(bytes: u64) -> String {
    const GIB: u64 = 1 << 30;
    const MIB: u64 = 1 << 20;
    const KIB: u64 = 1 << 10;
    let fixed = |v: u64, unit: u64, name: &str| {
        let whole = v / unit;
        let tenth = v % unit * 10 / unit;
        if tenth == 0 {
            format!("{whole} {name}")
        } else {
            format!("{whole}.{tenth} {name}")
        }
    };
    match bytes {
        b if b >= GIB => fixed(b, GIB, "GiB"),
        b if b >= MIB => fixed(b, MIB, "MiB"),
        b if b >= KIB => fixed(b, KIB, "KiB"),
        b => format!("{b} B"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_are_lowercase_words() {
        assert!(identifier_issue("analytics_restore", "database").is_none());
        assert!(identifier_issue("", "database").is_some());
        assert!(identifier_issue("Analytics", "database").is_some());
        assert!(identifier_issue("9lives", "database").is_some());
        assert!(identifier_issue("a-b", "database").is_some());
    }

    #[test]
    fn dates_parse_and_reject_rollover() {
        assert_eq!(
            parse_date("2026-12-31"),
            Some(crate::models::format::epoch(2026, 12, 31, 0, 0, 0))
        );
        assert_eq!(parse_date("2026-02-31"), None);
        assert_eq!(parse_date("2026-13-01"), None);
        assert_eq!(parse_date("soon"), None);
    }

    #[test]
    fn sizes_read_in_binary_units() {
        assert_eq!(bytes_text(18 << 30), "18 GiB");
        assert_eq!(bytes_text(3 << 29), "1.5 GiB");
        assert_eq!(bytes_text(512 << 20), "512 MiB");
        assert_eq!(bytes_text(12), "12 B");
    }
}
