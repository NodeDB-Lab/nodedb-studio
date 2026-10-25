//! Shell chrome models: nav-rail badge counts and the statusbar's session
//! summary. Both are single values (not lists), which is why the mock impl
//! cannot use `services::mock_behavior::apply` and instead uses
//! `apply_one`, the single-value counterpart `record_detail` also uses.

use serde::{Deserialize, Serialize};

/// The Studio version, from the crate manifest.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Badge counts shown on sidebar entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavBadges {
    /// Open query tabs.
    pub query: u32,
}

/// Server facts about the active session that the connect result does not
/// carry, shown in the footer. Role, current database and read-only stay
/// on `ActiveConnection`: one owner per fact, so the footer and the
/// account menu cannot show two different roles. The time zone is a
/// Studio preference, not a server fact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInfo {
    pub server_version: String,
}

/// What a recent item opens again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecentKind {
    GraphQuery,
    SqlQuery,
}

impl RecentKind {
    /// Words the palette matches besides the title.
    pub fn keywords(self) -> &'static [&'static str] {
        match self {
            RecentKind::GraphQuery => &["graph", "cypher", "query"],
            RecentKind::SqlQuery => &["sql", "query"],
        }
    }
}

/// An item the user opened lately, listed under Recent in the palette.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentItem {
    pub id: String,
    pub title: String,
    pub kind: RecentKind,
}
