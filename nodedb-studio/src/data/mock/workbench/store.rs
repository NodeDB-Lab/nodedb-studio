//! The mock's workbench state: the transaction, the saved-query library,
//! and the console history. Writes land here and every later read sees
//! them.

use std::collections::BTreeMap;

use crate::data::mock::workbench::console::{self, bare};
use crate::data::mock::workbench::library::{self, OWNER};
use crate::data::mock::workbench::results::{affected, result_set};
use crate::models::console::{ConsoleOutput, ConsoleReply, HistoryEntry, HistoryQuery, Protocol};
use crate::models::format::epoch;
use crate::models::saved_query::{
    QueryLibrary, QueryScope, SaveQuery, SavedQuery, SavedQueryVersion,
};
use crate::models::sql_text::{TokenKind, parameters, tokenize};
use crate::models::workbench::{
    MessageLevel, QueryMessage, QueryRequest, QueryRun, TxAccess, TxState,
};
use crate::services::error::StudioError;

#[derive(Debug, Clone)]
pub struct WorkbenchStore {
    tx: TxState,
    library: QueryLibrary,
    versions: BTreeMap<String, Vec<SavedQueryVersion>>,
    history: Vec<HistoryEntry>,
    /// The mock's clock. Every write advances it a minute, so timestamps
    /// are deterministic.
    clock: i64,
    seq: u32,
}

impl WorkbenchStore {
    pub fn seeded() -> Self {
        let library = library::library();
        let versions = library::versions(&library);
        Self {
            tx: TxState::Idle,
            library,
            versions,
            history: console::history(),
            clock: epoch(2026, 6, 13, 13, 30, 0),
            seq: 0,
        }
    }

    fn tick(&mut self) -> i64 {
        self.clock += 60;
        self.clock
    }

    fn next(&mut self) -> u32 {
        self.seq += 1;
        self.seq
    }

    pub fn transaction(&self) -> TxState {
        self.tx
    }

    pub fn begin(&mut self, access: TxAccess) -> Result<TxState, StudioError> {
        if self.tx.is_open() {
            return Err(StudioError::TransactionOpen);
        }
        self.tx = TxState::Active {
            access,
            statements: 0,
        };
        Ok(self.tx)
    }

    /// Commit. A failed transaction rolls back instead, as the server does.
    pub fn commit(&mut self) -> Result<TxState, StudioError> {
        if !self.tx.is_open() {
            return Err(StudioError::NoTransaction);
        }
        self.tx = TxState::Idle;
        Ok(self.tx)
    }

    pub fn rollback(&mut self) -> Result<TxState, StudioError> {
        if !self.tx.is_open() {
            return Err(StudioError::NoTransaction);
        }
        self.tx = TxState::Idle;
        Ok(self.tx)
    }

    /// An error inside an open transaction fails it.
    fn fail<T>(&mut self, e: StudioError) -> Result<T, StudioError> {
        if self.tx.is_open() {
            self.tx = TxState::Failed;
        }
        Err(e)
    }

    /// Count a statement toward the open transaction.
    fn counted(&mut self) {
        if let TxState::Active { access, statements } = self.tx {
            self.tx = TxState::Active {
                access,
                statements: statements + 1,
            };
        }
    }

    fn check(&self, request: &QueryRequest) -> Result<(), StudioError> {
        let sql = request.sql.trim();
        if sql.is_empty() {
            return Err(StudioError::EmptyStatement);
        }
        let mut depth = 0i32;
        for t in tokenize(sql, Protocol::Sql) {
            match t.kind {
                TokenKind::Str if t.text.len() < 2 || !t.text.ends_with('\'') => {
                    return Err(StudioError::Syntax {
                        reason: "a string is not closed".to_string(),
                    });
                }
                TokenKind::Plain => {
                    for c in t.text.chars() {
                        match c {
                            '(' => depth += 1,
                            ')' => depth -= 1,
                            _ => {}
                        }
                        if depth < 0 {
                            return Err(StudioError::Syntax {
                                reason: "a `)` has no matching `(`".to_string(),
                            });
                        }
                    }
                }
                TokenKind::Str
                | TokenKind::Keyword
                | TokenKind::Function
                | TokenKind::Number
                | TokenKind::Comment
                | TokenKind::Param
                | TokenKind::Type
                | TokenKind::Ident => {}
            }
        }
        if depth != 0 {
            return Err(StudioError::Syntax {
                reason: "a `(` is not closed".to_string(),
            });
        }
        for name in parameters(sql) {
            let bound = request
                .params
                .iter()
                .any(|p| p.name == name && !p.value.trim().is_empty());
            if !bound {
                return Err(StudioError::MissingParameter(name));
            }
        }
        Ok(())
    }

    /// Run one statement in the session's transaction, if one is open.
    pub fn run(&mut self, request: &QueryRequest) -> Result<QueryRun, StudioError> {
        if self.tx == TxState::Failed {
            return Err(StudioError::Rejected {
                reason: "the transaction failed: roll it back before running more statements"
                    .to_string(),
            });
        }
        if let Err(e) = self.check(request) {
            return self.fail(e);
        }
        let first = request
            .sql
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_uppercase();
        if matches!(first.as_str(), "INSERT" | "UPDATE" | "DELETE") {
            if matches!(
                self.tx,
                TxState::Active {
                    access: TxAccess::ReadOnly,
                    ..
                }
            ) {
                return self.fail(StudioError::ReadOnlyTransaction);
            }
            self.counted();
            let tag = if first == "INSERT" {
                "INSERT 0 1"
            } else {
                "1 row changed"
            };
            return Ok(QueryRun {
                result: affected(1),
                messages: vec![QueryMessage {
                    level: MessageLevel::Info,
                    text: tag.to_string(),
                }],
                tx: self.tx,
            });
        }
        self.counted();
        let result = result_set(request);
        let mut messages = vec![QueryMessage {
            level: MessageLevel::Info,
            text: format!("{} rows in {} ms", result.total_rows, result.elapsed_ms),
        }];
        if let TxState::Active { access, .. } = self.tx {
            messages.push(QueryMessage {
                level: MessageLevel::Info,
                text: format!(
                    "Ran inside a {} transaction. Nothing is final until Commit.",
                    access.label()
                ),
            });
        }
        Ok(QueryRun {
            result,
            messages,
            tx: self.tx,
        })
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
            return Err(StudioError::Rejected {
                reason: "a saved query needs a name".to_string(),
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
            return Err(StudioError::Rejected {
                reason: format!("there is no folder named `{folder}`"),
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
                if q.owner != OWNER {
                    return Err(StudioError::Rejected {
                        reason: format!("only {} can save a new version: use Save as", q.owner),
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
                    owner: OWNER.to_string(),
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
                author: OWNER.to_string(),
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
            return Err(StudioError::Rejected {
                reason: "a folder needs a name".to_string(),
            });
        }
        if self
            .library
            .folders
            .iter()
            .any(|f| f.eq_ignore_ascii_case(name))
        {
            return Err(StudioError::Rejected {
                reason: format!("a folder named `{name}` already exists"),
            });
        }
        self.library.folders.push(name.to_string());
        Ok(())
    }

    /// Run one console command. Transaction control moves the session's
    /// transaction, the same one the Query page shows.
    pub fn console(
        &mut self,
        protocol: Protocol,
        database: &str,
        text: &str,
    ) -> Result<ConsoleReply, StudioError> {
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
        let control = if protocol == Protocol::Sql {
            match bare(text).to_uppercase().as_str() {
                "BEGIN" | "START TRANSACTION" => Some(("BEGIN", self.begin(TxAccess::ReadWrite))),
                "COMMIT" => Some(("COMMIT", self.commit())),
                "ROLLBACK" => Some(("ROLLBACK", self.rollback())),
                _ => None,
            }
        } else {
            None
        };
        if let Some((tag, outcome)) = control {
            let tx = outcome?;
            return Ok(ConsoleReply {
                output: ConsoleOutput::Text(vec![tag.to_string()]),
                elapsed_ms: 1,
                tx,
            });
        }
        if self.tx == TxState::Failed {
            return Err(StudioError::Rejected {
                reason: "the transaction failed: ROLLBACK before running more commands".to_string(),
            });
        }
        match console::execute(protocol, database, text) {
            Ok((output, elapsed_ms)) => {
                self.counted();
                Ok(ConsoleReply {
                    output,
                    elapsed_ms,
                    tx: self.tx,
                })
            }
            Err(e) => self.fail(e),
        }
    }

    pub fn history(&self, query: &HistoryQuery) -> Vec<HistoryEntry> {
        let since = query.window.seconds().map(|s| self.clock - s);
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
