//! Notifications surfaced through the topbar bell.
//!
//! Each notification may declare a `required_cap`: notifications about
//! features the active connection doesn't support are filtered out before
//! the bell badge count is computed. See `state::notifications`.

use serde::{Deserialize, Serialize};

use crate::state::connection::Capability;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Info,
    Warn,
    Err,
}

impl Severity {
    /// CSS modifier class ported from the mockup (`.sev.info` etc.).
    pub fn css_class(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warn => "warn",
            Severity::Err => "err",
        }
    }
}

/// Where clicking a notification navigates. Kept deliberately narrow — only
/// the targets the mock notifications actually use. Maps onto the studio
/// `Route` once routing lands (Phase 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationTarget {
    Sync,
    StreamsTopics,
    StreamsCron,
    Admin,
    Query,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    pub severity: Severity,
    /// Heading the bell list groups items under (e.g. "Sync conflicts").
    pub group: String,
    /// If set, hidden unless the active connection has this capability.
    pub required_cap: Option<Capability>,
    pub title: String,
    pub desc: String,
    /// Relative timestamp string, pre-rendered for the mock (e.g. "2m ago").
    pub when: String,
    pub target: NotificationTarget,
    pub unread: bool,
}
