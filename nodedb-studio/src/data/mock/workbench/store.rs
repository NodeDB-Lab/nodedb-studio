//! The mock's workbench state: the transaction, and the seeded saved-query
//! library and console history. Writes land here and every later read sees
//! them.

use crate::data::mock::workbench::console::{self, bare};
use crate::data::mock::workbench::library::{self, OWNER};
use crate::data::mock::workbench::results::{affected, result_set};
use crate::models::format::epoch;
use crate::models::workbench::console::{ConsoleOutput, ConsoleReply, Protocol};
use crate::models::workbench::sql_text::{TokenKind, parameters, tokenize};
use crate::models::workbench::{
    MessageLevel, QueryMessage, QueryRequest, QueryRun, TxAccess, TxState,
};
use crate::services::error::StudioError;
use crate::state::workbench_records::WorkbenchRecords;

#[derive(Debug, Clone)]
pub struct WorkbenchStore {
    tx: TxState,
    /// The seeded library and history. The mock's clock advances a minute
    /// per write, so timestamps are deterministic.
    pub records: WorkbenchRecords,
}

impl WorkbenchStore {
    pub fn seeded() -> Self {
        let library = library::library();
        let versions = library::versions(&library);
        Self {
            tx: TxState::Idle,
            records: WorkbenchRecords::ticking(
                library,
                versions,
                console::history(),
                epoch(2026, 6, 13, 13, 30, 0),
                OWNER,
            ),
        }
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
            return Err(StudioError::Refused {
                reason: "The transaction failed. Roll it back before running more statements."
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

    /// Run one console command. Transaction control moves the session's
    /// transaction, the same one the Query page shows.
    pub fn console(
        &mut self,
        protocol: Protocol,
        database: &str,
        text: &str,
    ) -> Result<ConsoleReply, StudioError> {
        self.records.record(protocol, text);
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
                notes: Vec::new(),
            });
        }
        if self.tx == TxState::Failed {
            return Err(StudioError::Refused {
                reason: "The transaction failed. Run ROLLBACK before running more commands."
                    .to_string(),
            });
        }
        match console::execute(protocol, database, text) {
            Ok((output, elapsed_ms)) => {
                self.counted();
                Ok(ConsoleReply {
                    output,
                    elapsed_ms,
                    tx: self.tx,
                    notes: Vec::new(),
                })
            }
            Err(e) => self.fail(e),
        }
    }
}
