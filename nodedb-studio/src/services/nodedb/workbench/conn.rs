//! The session's statement connections: one per database, outside the
//! pool. Taking one out for a call, putting it back, closing it, awaiting a
//! call within a time limit, and what a call without an answer means.
//!
//! The server keeps settings (`SET`), sequence values (`currval`) and the
//! transaction per connection. The pool hands each call whichever
//! connection is free, so every statement the user types runs on the
//! statement connection of its database instead. Each database has its own,
//! with its own settings. A transaction opens on it and leaves it open after
//! COMMIT or ROLLBACK. Studio's own reads keep using the pool.
//!
//! No `RefCell` borrow lives across an `.await`. A call takes the
//! connection out of the session, awaits, and puts it back. A call that
//! needs the connection meanwhile is refused, never queued.

use std::future::Future;
use std::time::{Duration, Instant};

use nodedb_client::native::connection::NativeConnection;
use nodedb_client::native::pool::PoolConfig;
use nodedb_client::{NodeDbError, QueryResult};
use nodedb_types::Value;
use nodedb_types::error::{ErrorCode, ErrorDetails};

use crate::models::workbench::TxState;
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;
use crate::services::nodedb::service::{Session, next_id};
use crate::services::nodedb::workbench::tx::Tx;

/// A failed block refuses every statement except transaction control. This
/// read answers in an open block and is refused in a failed one.
const PROBE: &str = "SHOW transaction_read_only";

/// A connection idle this long is checked with the server before its next
/// call. 0.6.2 closes a native session idle for `idle_timeout_secs` (3600 s
/// by default), and rolls its transaction back.
pub(in crate::services::nodedb) const IDLE_PROBE_AFTER: Duration = Duration::from_secs(30);

/// Shown with the first statement on a connection that replaces one that
/// closed.
pub(in crate::services::nodedb) const SETTINGS_RESET: &str = "Studio's connection to this database had closed, so this statement ran on a new one. Settings made with SET are back to their defaults, and currval() has no value until the next nextval().";

/// How long one statement on a statement connection can run.
pub(in crate::services::nodedb) const STATEMENT_LIMIT: Duration = Duration::from_secs(300);

/// How long BEGIN, COMMIT, ROLLBACK and the probe can take.
pub(in crate::services::nodedb) const CONTROL_LIMIT: Duration = Duration::from_secs(60);

/// The tag of a status row, such as `BEGIN` or `COMMIT`.
pub(in crate::services::nodedb) fn status_of(result: &QueryResult) -> Option<&str> {
    match (result.columns.as_slice(), result.rows.as_slice()) {
        ([column], [row]) if column == "status" => row.first().and_then(Value::as_str),
        _ => None,
    }
}

/// The connection is gone: the client could not write or read the frame.
pub(in crate::services::nodedb) fn is_dead(e: &NodeDbError) -> bool {
    matches!(e.details(), ErrorDetails::SyncConnectionFailed)
}

/// No answer came: the connection died, or the call ran out of time. What
/// the server did with the call is unknown.
#[derive(Debug)]
pub(in crate::services::nodedb) struct Gone(pub String);

/// Await `call` for at most `limit`. The server's answer, success or
/// error, is `Ok`.
pub(in crate::services::nodedb) async fn bounded(
    limit: Duration,
    call: impl Future<Output = Result<QueryResult, NodeDbError>>,
) -> Result<Result<QueryResult, NodeDbError>, Gone> {
    match tokio::time::timeout(limit, call).await {
        Err(_) => Err(Gone(format!(
            "no answer within {limit:?}, so Studio closed the connection"
        ))),
        Ok(Err(e)) if is_dead(&e) => Err(Gone(e.to_string())),
        Ok(answer) => Ok(answer),
    }
}

/// The transaction ended with its connection. Nothing it wrote is committed.
pub(in crate::services::nodedb) fn lost(detail: impl std::fmt::Display) -> StudioError {
    StudioError::Connection(NodeDbError::sync_connection_failed(format!(
        "the transaction ended and its uncommitted statements are gone: {detail}"
    )))
}

/// COMMIT went out and no answer came back: the server can have committed.
/// Not retriable, so no Retry offers to commit again.
fn commit_unknown(detail: &str) -> StudioError {
    StudioError::Connection(NodeDbError::from_wire(
        ErrorCode::INTERNAL,
        format!(
            "COMMIT got no answer ({detail}). The server can have committed the transaction: check the data before you run its statements again."
        ),
    ))
}

/// A statement outside a transaction went out and no answer came back. Not
/// retriable, so no Retry runs a write a second time.
pub(in crate::services::nodedb) fn no_answer(detail: impl std::fmt::Display) -> StudioError {
    StudioError::Connection(NodeDbError::from_wire(
        ErrorCode::INTERNAL,
        format!(
            "the statement got no answer ({detail}). It can have run: check the data before you run it again."
        ),
    ))
}

/// A second call while one runs on the same statement connection.
pub(in crate::services::nodedb) fn busy() -> StudioError {
    StudioError::Refused {
        reason: "A statement is still running on this database's connection. Wait for it to finish, then try again.".to_string(),
    }
}

/// A connect replaced the session while a call ran on its connection.
fn replaced() -> StudioError {
    StudioError::Connection(NodeDbError::from_wire(
        ErrorCode::INTERNAL,
        "Studio connected again while the statement ran, and closed the connection it ran on. It can have run: check the data before you run it again.",
    ))
}

/// The answer to the COMMIT or ROLLBACK that ended a transaction.
pub(in crate::services::nodedb) fn ended(
    committing: bool,
    answer: Result<Result<QueryResult, NodeDbError>, Gone>,
) -> Result<QueryResult, StudioError> {
    match answer {
        Ok(Ok(result)) => Ok(result),
        // The server refused: a refused COMMIT rolled back, and the closed
        // connection rolls back anything left.
        Ok(Err(e)) => Err(e.into()),
        Err(Gone(detail)) if committing => Err(commit_unknown(&detail)),
        Err(Gone(detail)) => Err(lost(detail)),
    }
}

/// Whether the server holds the transaction failed.
pub(in crate::services::nodedb) async fn failed_on(
    conn: &mut NativeConnection,
) -> Result<bool, Gone> {
    Ok(bounded(CONTROL_LIMIT, conn.execute_sql(PROBE))
        .await?
        .is_err())
}

/// Open a statement connection, as the pool would.
pub(in crate::services::nodedb) async fn open(
    config: &PoolConfig,
) -> Result<NativeConnection, StudioError> {
    let connect = async {
        let mut conn = if config.tls.enabled {
            NativeConnection::connect_tls(&config.addr, &config.tls).await?
        } else {
            NativeConnection::connect(&config.addr).await?
        };
        conn.authenticate(config.auth.clone(), config.database.as_deref())
            .await?;
        Ok::<_, NodeDbError>(conn)
    };
    tokio::time::timeout(config.connect_timeout, connect)
        .await
        .map_err(|_| NodeDbError::sync_connection_failed("connect timeout"))?
        .map_err(StudioError::from)
}

/// One database's statement connection.
pub(in crate::services::nodedb) struct StmtConn {
    /// Tells this connection from the one a reconnect opens.
    pub id: u64,
    /// `None` while a call runs on it.
    pub conn: Option<NativeConnection>,
    /// When the last call on it finished.
    pub used: Instant,
}

impl StmtConn {
    pub fn idle(&self) -> bool {
        self.used.elapsed() >= IDLE_PROBE_AFTER
    }
}

/// A statement connection out of the session while a call runs on it.
/// Dropped without being put back (a cancelled call), it closes the
/// connection: the server rolls back an open transaction and forgets the
/// session's settings.
pub(in crate::services::nodedb) struct Lease<'a> {
    svc: &'a NodeDbConnectionService,
    session: u64,
    database: String,
    id: u64,
    /// The transaction open on this connection when the call took it.
    pub tx: Option<u64>,
    conn: Option<NativeConnection>,
    /// The connection replaces one that closed: the settings made there
    /// are gone.
    pub reset: bool,
}

impl Lease<'_> {
    pub fn conn(&mut self) -> Result<&mut NativeConnection, StudioError> {
        self.conn.as_mut().ok_or_else(busy)
    }

    pub fn database(&self) -> &str {
        &self.database
    }

    /// Put the connection back after `update` accepts the transaction
    /// change, and read the transaction state. A refused change closes the
    /// connection instead: what it holds on the server is not what Studio
    /// shows.
    pub fn put_back(
        mut self,
        update: impl FnOnce(&mut Option<Tx>) -> Result<(), StudioError>,
    ) -> Result<TxState, StudioError> {
        let conn = self.conn.take();
        let mut session = self.svc.session.borrow_mut();
        let Some(s) = session.as_mut().filter(|s| s.id == self.session) else {
            return Err(replaced());
        };
        if let Err(e) = update(&mut s.tx) {
            s.close(&self.database, self.id);
            return Err(e);
        }
        if let Some(slot) = s.conns.get_mut(&self.database).filter(|c| c.id == self.id) {
            slot.conn = conn;
            slot.used = Instant::now();
        }
        Ok(s.tx.as_ref().map_or(TxState::Idle, Tx::state))
    }

    /// Close the connection. The server rolls back an open transaction.
    pub fn end(mut self) {
        self.conn = None;
        self.svc.close(self.session, &self.database, self.id);
    }
}

impl Drop for Lease<'_> {
    fn drop(&mut self) {
        if self.conn.take().is_some() {
            self.svc.close(self.session, &self.database, self.id);
        }
    }
}

impl Session {
    /// Forget connection `id` of `database`, and the transaction on it. The
    /// next statement there opens a new one and says the settings are gone.
    pub(in crate::services::nodedb) fn close(&mut self, database: &str, id: u64) {
        if self.conns.get(database).is_some_and(|c| c.id == id) {
            self.conns.remove(database);
            self.reset.insert(database.to_string());
            if self.tx.as_ref().is_some_and(|tx| tx.database() == database) {
                self.tx = None;
            }
        }
    }
}

impl NodeDbConnectionService {
    fn close(&self, session: u64, database: &str, id: u64) {
        // `try_` because this runs from `Drop`. Every borrow of the session
        // is short and never held across an await, so it succeeds.
        if let Ok(mut s) = self.session.try_borrow_mut()
            && let Some(s) = s.as_mut().filter(|s| s.id == session)
        {
            s.close(database, id);
        }
    }

    /// Take `database`'s connection out of `s`. `None` when it has none or
    /// a call runs on it.
    pub(in crate::services::nodedb) fn checkout(
        &self,
        s: &mut Session,
        database: &str,
    ) -> Option<Lease<'_>> {
        let tx =
            s.tx.as_ref()
                .filter(|tx| tx.database() == database)
                .map(Tx::id);
        let slot = s.conns.get_mut(database)?;
        let conn = slot.conn.take()?;
        Some(Lease {
            svc: self,
            session: s.id,
            database: database.to_string(),
            id: slot.id,
            tx,
            conn: Some(conn),
            reset: false,
        })
    }

    /// Take the statement connection of `database` (blank: the session's).
    /// `check` sees the database and the open transaction first, and can
    /// refuse before anything reaches the server. A database without a
    /// connection opens one. A connection idle past `IDLE_PROBE_AFTER` is
    /// asked first: a dead one ends its transaction, or outside one is
    /// replaced.
    pub(in crate::services::nodedb) async fn take(
        &self,
        database: &str,
        check: impl FnOnce(&str, Option<&Tx>) -> Result<(), StudioError>,
    ) -> Result<Lease<'_>, StudioError> {
        let (session, database, held, config) = {
            let mut guard = self.session.borrow_mut();
            let s = guard.as_mut().ok_or(StudioError::NotConnected)?;
            let database = s.database_or_default(database);
            check(&database, s.tx.as_ref())?;
            let held = match s.conns.get(&database) {
                Some(slot) => {
                    let idle = slot.idle();
                    Some((self.checkout(s, &database).ok_or_else(busy)?, idle))
                }
                // A transaction lives on its database's connection.
                None if s.tx.as_ref().is_some_and(|tx| tx.database() == database) => {
                    s.tx = None;
                    return Err(lost("Studio no longer held its connection"));
                }
                None => None,
            };
            let config = PoolConfig {
                database: Some(database.clone()),
                ..s.config.clone()
            };
            (s.id, database, held, config)
        };
        if let Some((mut lease, idle)) = held {
            if !idle {
                return Ok(lease);
            }
            match failed_on(lease.conn()?).await {
                Ok(_) => return Ok(lease),
                Err(Gone(detail)) => {
                    let in_tx = lease.tx.is_some();
                    lease.end();
                    if in_tx {
                        return Err(lost(detail));
                    }
                }
            }
        }
        let conn = open(&config).await?;
        let mut guard = self.session.borrow_mut();
        let Some(s) = guard.as_mut().filter(|s| s.id == session) else {
            return Err(StudioError::Connection(
                NodeDbError::sync_connection_failed(
                    "Studio connected again before the statement started, so it did not run",
                ),
            ));
        };
        // Another call opened one first.
        if s.conns.contains_key(&database) {
            return Err(busy());
        }
        let id = next_id();
        s.conns.insert(
            database.clone(),
            StmtConn {
                id,
                conn: None,
                used: Instant::now(),
            },
        );
        let reset = s.reset.remove(&database);
        Ok(Lease {
            svc: self,
            session,
            database,
            id,
            tx: None,
            conn: Some(conn),
            reset,
        })
    }
}

/// Kill the statement connection of `database`, as a server that closed it
/// leaves it. `idle` makes it look unused for longer than
/// `IDLE_PROBE_AFTER`. A frame over the server's 16 MiB limit makes the
/// server answer an error and close the connection.
#[cfg(test)]
pub(in crate::services::nodedb) async fn kill_conn(
    svc: &NodeDbConnectionService,
    database: &str,
    idle: bool,
) {
    let mut conn = svc
        .session
        .borrow_mut()
        .as_mut()
        .and_then(|s| s.conns.get_mut(database))
        .and_then(|c| c.conn.take())
        .expect("a statement connection");
    let huge = format!("SELECT '{}'", "x".repeat(17 * 1024 * 1024));
    let out = tokio::time::timeout(Duration::from_secs(30), conn.execute_sql(&huge))
        .await
        .expect("the server answers in time");
    assert!(
        out.as_ref().is_err_and(is_dead),
        "the connection is dead: {out:?}"
    );
    let mut session = svc.session.borrow_mut();
    let slot = session
        .as_mut()
        .and_then(|s| s.conns.get_mut(database))
        .expect("still held");
    slot.conn = Some(conn);
    if idle {
        slot.used = Instant::now()
            .checked_sub(IDLE_PROBE_AFTER * 2)
            .expect("the clock runs long enough");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_status_row_names_its_tag() {
        let row = |cols: &[&str], tag: &str| QueryResult {
            columns: cols.iter().map(|c| c.to_string()).collect(),
            rows: vec![vec![Value::String(tag.into())]],
            rows_affected: 1,
            command: None,
        };
        assert_eq!(status_of(&row(&["status"], "BEGIN")), Some("BEGIN"));
        assert_eq!(status_of(&row(&["id"], "BEGIN")), None);
    }

    fn dead() -> NodeDbError {
        NodeDbError::sync_connection_failed("I/O: early eof")
    }

    const SHORT: Duration = Duration::from_millis(10);

    #[tokio::test]
    async fn a_dead_or_silent_connection_is_gone() {
        let died = bounded(CONTROL_LIMIT, async { Err(dead()) }).await;
        assert!(
            matches!(&died, Err(Gone(d)) if d.contains("early eof")),
            "{died:?}"
        );
        let silent = bounded(SHORT, std::future::pending()).await;
        assert!(
            matches!(&silent, Err(Gone(d)) if d.contains("no answer within 10ms")),
            "{silent:?}"
        );
        let refused = bounded(CONTROL_LIMIT, async { Err(NodeDbError::bad_request("no")) }).await;
        assert!(matches!(refused, Ok(Err(_))), "the server's own answer");
    }

    /// A COMMIT that got no answer reads as an unknown outcome, never as a
    /// rollback, and offers no Retry.
    #[tokio::test]
    async fn a_commit_without_an_answer_has_an_unknown_outcome() {
        let died = ended(true, bounded(CONTROL_LIMIT, async { Err(dead()) }).await);
        let silent = ended(true, bounded(SHORT, std::future::pending()).await);
        for e in [died, silent] {
            let e = e.expect_err("no answer");
            assert!(matches!(e, StudioError::Connection(_)), "{e:?}");
            assert!(!e.is_retriable(), "a Retry would commit again");
            let text = e.to_string();
            assert!(
                text.contains("COMMIT got no answer")
                    && text.contains("can have committed")
                    && text.contains("check the data"),
                "{text}"
            );
            assert!(!text.contains("uncommitted statements are gone"), "{text}");
        }
        // A ROLLBACK without an answer: the closed connection rolled back.
        let rolled = ended(false, bounded(CONTROL_LIMIT, async { Err(dead()) }).await)
            .expect_err("no answer");
        assert!(
            rolled
                .to_string()
                .contains("uncommitted statements are gone")
        );
        // The server's refusal of a COMMIT is its own error.
        let refused =
            ended(true, Ok(Err(NodeDbError::bad_request("refused")))).expect_err("refused");
        assert!(matches!(refused, StudioError::Setup(_)), "{refused:?}");
    }
}
