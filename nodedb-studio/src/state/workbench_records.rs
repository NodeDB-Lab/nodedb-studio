//! Studio's own workbench records: the saved-query library with each
//! query's versions, and the console history. Every backend keeps them the
//! same way. Writes land here and every later read sees them.

use std::collections::BTreeMap;

use crate::models::workbench::console::{HistoryEntry, HistoryQuery, Protocol};
use crate::models::workbench::saved_query::{
    QueryLibrary, QueryScope, SaveQuery, SavedQuery, SavedQueryVersion,
};
use crate::services::error::StudioError;

#[derive(Debug, Clone)]
pub struct WorkbenchRecords {
    library: QueryLibrary,
    versions: BTreeMap<String, Vec<SavedQueryVersion>>,
    history: Vec<HistoryEntry>,
    /// The time of the last write. Without `now`, every write advances it a
    /// minute, so timestamps are deterministic.
    clock: i64,
    seq: u32,
    /// Who saves: the owner of a new query and the author of a version.
    owner: String,
    /// The wall clock. `None` keeps the minute ticks.
    now: Option<fn() -> i64>,
}

impl WorkbenchRecords {
    /// Records with no queries, folders, or history, owned by `owner` and
    /// timed by `now`.
    pub fn empty(owner: &str, now: fn() -> i64) -> Self {
        Self {
            library: QueryLibrary::default(),
            versions: BTreeMap::new(),
            history: Vec::new(),
            clock: now(),
            seq: 0,
            owner: owner.to_string(),
            now: Some(now),
        }
    }

    /// Records that start with `library`, its `versions`, and `history`,
    /// owned by `owner`. Each write advances `clock` by a minute.
    pub fn ticking(
        library: QueryLibrary,
        versions: BTreeMap<String, Vec<SavedQueryVersion>>,
        history: Vec<HistoryEntry>,
        clock: i64,
        owner: &str,
    ) -> Self {
        Self {
            library,
            versions,
            history,
            clock,
            seq: 0,
            owner: owner.to_string(),
            now: None,
        }
    }

    fn tick(&mut self) -> i64 {
        self.clock = match self.now {
            Some(now) => now().max(self.clock),
            None => self.clock + 60,
        };
        self.clock
    }

    fn next(&mut self) -> u32 {
        self.seq += 1;
        self.seq
    }

    pub fn library(&self) -> QueryLibrary {
        self.library.clone()
    }

    pub fn versions(&self, id: &str) -> Result<Vec<SavedQueryVersion>, StudioError> {
        self.versions
            .get(id)
            .cloned()
            .ok_or_else(|| StudioError::UnknownQuery(id.to_string()))
    }

    pub fn save(&mut self, save: &SaveQuery) -> Result<SavedQuery, StudioError> {
        let name = save.name.trim().to_string();
        if name.is_empty() {
            return Err(StudioError::Refused {
                reason: "A saved query needs a name. Enter one.".to_string(),
            });
        }
        if self
            .library
            .queries
            .iter()
            .any(|q| q.name.eq_ignore_ascii_case(&name) && Some(&q.id) != save.id.as_ref())
        {
            return Err(StudioError::QueryNameTaken(name));
        }
        if let Some(folder) = &save.folder
            && !self.library.folders.contains(folder)
        {
            return Err(StudioError::Refused {
                reason: format!("There is no folder named `{folder}`. Pick a folder that exists."),
            });
        }
        let now = self.tick();
        let saved = match &save.id {
            Some(id) => {
                let idx = self
                    .library
                    .queries
                    .iter()
                    .position(|q| &q.id == id)
                    .ok_or_else(|| StudioError::UnknownQuery(id.clone()))?;
                let q = &mut self.library.queries[idx];
                if q.owner != self.owner {
                    return Err(StudioError::Refused {
                        reason: format!("Only {} can save a new version. Use Save as.", q.owner),
                    });
                }
                q.name = name;
                q.folder = save.folder.clone();
                q.database = save.database.clone();
                q.sql = save.sql.clone();
                q.params = save.params.clone();
                q.tags = save.tags.clone();
                q.version += 1;
                q.updated_at = now;
                q.last_error = None;
                q.clone()
            }
            None => {
                let n = self.next();
                let q = SavedQuery {
                    id: format!("q-new-{n}"),
                    name,
                    folder: save.folder.clone(),
                    scope: QueryScope::MyWorkspace,
                    owner: self.owner.clone(),
                    database: save.database.clone(),
                    sql: save.sql.clone(),
                    params: save.params.clone(),
                    tags: save.tags.clone(),
                    version: 1,
                    updated_at: now,
                    last_error: None,
                };
                self.library.queries.push(q.clone());
                q
            }
        };
        self.versions.entry(saved.id.clone()).or_default().insert(
            0,
            SavedQueryVersion {
                version: saved.version,
                saved_at: now,
                author: self.owner.clone(),
                sql: saved.sql.clone(),
            },
        );
        Ok(saved)
    }

    pub fn delete(&mut self, id: &str) -> Result<(), StudioError> {
        let before = self.library.queries.len();
        self.library.queries.retain(|q| q.id != id);
        if self.library.queries.len() == before {
            return Err(StudioError::UnknownQuery(id.to_string()));
        }
        self.versions.remove(id);
        Ok(())
    }

    pub fn create_folder(&mut self, name: &str) -> Result<(), StudioError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(StudioError::Refused {
                reason: "A folder needs a name. Enter one.".to_string(),
            });
        }
        if self
            .library
            .folders
            .iter()
            .any(|f| f.eq_ignore_ascii_case(name))
        {
            return Err(StudioError::Refused {
                reason: format!("A folder named `{name}` already exists. Choose another name."),
            });
        }
        self.library.folders.push(name.to_string());
        Ok(())
    }

    /// Add a console command to the history, newest first.
    pub fn record(&mut self, protocol: Protocol, text: &str) {
        let at = self.tick();
        let n = 100 + self.next();
        self.history.insert(
            0,
            HistoryEntry {
                id: format!("h-{n:03}"),
                at,
                protocol,
                text: text.trim().to_string(),
            },
        );
    }

    pub fn history(&self, query: &HistoryQuery) -> Vec<HistoryEntry> {
        let now = self.now.map_or(self.clock, |now| now());
        let since = query.window.seconds().map(|s| now - s);
        let mut out: Vec<HistoryEntry> = self
            .history
            .iter()
            .filter(|h| query.protocol.is_none_or(|p| p == h.protocol))
            .filter(|h| since.is_none_or(|s| h.at >= s))
            .cloned()
            .collect();
        out.sort_by_key(|h| std::cmp::Reverse(h.at));
        out
    }
}
