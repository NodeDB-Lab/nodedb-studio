//! Notifications in the bell drawer.
//!
//! Each entry has a typed kind, a scope (the object it concerns), and a
//! target page. An entry can declare a `required_cap`: the drawer and the
//! bell count skip entries about features the active connection lacks. See
//! `state::notifications`.

use serde::{Deserialize, Serialize};

use crate::routes::Route;
use crate::routes::tabs::{AdminTab, ClusterTab, EventsTab, SearchTab, SyncTab};
use crate::state::connection::Capability;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Success,
    Info,
    Warn,
    Err,
}

/// What happened. The kind fixes the icon family and the wording of the
/// action link through its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationKind {
    IndexBuild,
    SyncRejection,
    Backup,
    RoleChange,
    JobFailure,
    ReplicationLag,
}

/// What the object in an entry's scope line is, which picks its icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScopeKind {
    Collection,
    Database,
    Role,
    Job,
    Cluster,
}

/// The page an entry's action link opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationTarget {
    SearchIndexes,
    SyncRejectedWrites,
    AdminBackups,
    AdminAudit,
    EventsScheduledJobs,
    ClusterNodes,
}

impl NotificationTarget {
    /// The action link text.
    pub fn action_label(self) -> &'static str {
        match self {
            NotificationTarget::SearchIndexes => "View index",
            NotificationTarget::SyncRejectedWrites => "View rejection",
            NotificationTarget::AdminBackups => "View backup",
            NotificationTarget::AdminAudit => "View audit",
            NotificationTarget::EventsScheduledJobs => "View job",
            NotificationTarget::ClusterNodes => "View nodes",
        }
    }

    pub fn route(self) -> Route {
        match self {
            NotificationTarget::SearchIndexes => Route::Search {
                tab: SearchTab::Indexes,
            },
            NotificationTarget::SyncRejectedWrites => Route::Sync {
                tab: SyncTab::RejectedWrites,
            },
            NotificationTarget::AdminBackups => Route::Admin {
                tab: AdminTab::Backups,
            },
            NotificationTarget::AdminAudit => Route::Admin {
                tab: AdminTab::Audit,
            },
            NotificationTarget::EventsScheduledJobs => Route::Events {
                tab: EventsTab::ScheduledJobs,
            },
            NotificationTarget::ClusterNodes => Route::Cluster {
                tab: ClusterTab::Nodes,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    pub kind: NotificationKind,
    pub severity: Severity,
    /// If set, hidden unless the active connection has this capability.
    pub required_cap: Option<Capability>,
    pub title: String,
    pub body: String,
    /// The object the entry concerns, e.g. a collection name.
    pub scope: String,
    pub scope_kind: ScopeKind,
    /// When it happened, seconds since the epoch.
    pub at: i64,
    pub target: NotificationTarget,
    pub unread: bool,
}
