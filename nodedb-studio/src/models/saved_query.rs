//! Saved queries: named, persistent objects with an owner, a scope, tags,
//! and a version history. They are not tabs: a tab opens one.
//!
//! A saved query holds its SQL, its parameters, and the database it is
//! bound to. It never holds credentials.

use crate::models::workbench::ParamBinding;

/// Who can open a saved query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryScope {
    MyWorkspace,
    Shared,
}

impl QueryScope {
    pub fn label(self) -> &'static str {
        match self {
            QueryScope::MyWorkspace => "My workspace",
            QueryScope::Shared => "Shared",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedQuery {
    pub id: String,
    pub name: String,
    /// A folder of My queries. `None` lists it at the top.
    pub folder: Option<String>,
    pub scope: QueryScope,
    pub owner: String,
    pub database: String,
    pub sql: String,
    pub params: Vec<ParamBinding>,
    pub tags: Vec<String>,
    pub version: u32,
    /// Seconds since the epoch, UTC.
    pub updated_at: i64,
    /// The message of the last run if it failed.
    pub last_error: Option<String>,
}

/// One stored version of a saved query, newest first in a listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedQueryVersion {
    pub version: u32,
    pub saved_at: i64,
    pub author: String,
    pub sql: String,
}

/// The library: every folder and every saved query the session can open.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QueryLibrary {
    /// Folders of My queries, in creation order.
    pub folders: Vec<String>,
    pub queries: Vec<SavedQuery>,
}

impl QueryLibrary {
    pub fn get(&self, id: &str) -> Option<&SavedQuery> {
        self.queries.iter().find(|q| q.id == id)
    }

    /// Queries of My workspace that did not fail, outside any folder.
    pub fn mine(&self) -> Vec<&SavedQuery> {
        self.queries
            .iter()
            .filter(|q| {
                q.scope == QueryScope::MyWorkspace && q.last_error.is_none() && q.folder.is_none()
            })
            .collect()
    }

    /// Queries of My workspace that did not fail, in `folder`.
    pub fn in_folder(&self, folder: &str) -> Vec<&SavedQuery> {
        self.queries
            .iter()
            .filter(|q| {
                q.scope == QueryScope::MyWorkspace
                    && q.last_error.is_none()
                    && q.folder.as_deref() == Some(folder)
            })
            .collect()
    }

    pub fn shared(&self) -> Vec<&SavedQuery> {
        self.queries
            .iter()
            .filter(|q| q.scope == QueryScope::Shared && q.last_error.is_none())
            .collect()
    }

    /// Queries whose last run failed.
    pub fn recent_errors(&self) -> Vec<&SavedQuery> {
        self.queries
            .iter()
            .filter(|q| q.last_error.is_some())
            .collect()
    }

    /// Names that match `needle`, ignoring case. An empty needle matches all.
    pub fn matches(q: &SavedQuery, needle: &str) -> bool {
        let n = needle.trim().to_lowercase();
        n.is_empty()
            || q.name.to_lowercase().contains(&n)
            || q.tags.iter().any(|t| t.to_lowercase().contains(&n))
    }
}

/// A save: a new query when `id` is `None`, else a new version of `id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveQuery {
    pub id: Option<String>,
    pub name: String,
    pub folder: Option<String>,
    pub database: String,
    pub sql: String,
    pub params: Vec<ParamBinding>,
    pub tags: Vec<String>,
}
