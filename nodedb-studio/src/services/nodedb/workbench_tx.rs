//! The session's transaction: one server connection, pinned from BEGIN until
//! COMMIT or ROLLBACK.
//!
//! The server keeps a transaction per connection, and the pooled
//! `NativeClient` hands each call whichever connection is free. So a
//! transaction opens its own `NativeConnection` outside the pool, and every
//! statement on its database runs there. That connection never joins the
//! pool: it closes when the transaction ends, and the server rolls back what
//! a closed connection left open.
//!
//! No `RefCell` borrow lives across an `.await`. A statement takes the
//! connection out of the session, awaits, and puts it back. A call that
//! needs the connection meanwhile is refused, never queued. Reads outside the
//! transaction (the catalog, the schema tree, EXPLAIN) use the pool.

use std::rc::Rc;

use nodedb_client::native::connection::NativeConnection;
use nodedb_client::native::pool::PoolConfig;
use nodedb_client::{NativeClient, NodeDb, NodeDbError, QueryResult};
use nodedb_types::Value;
use nodedb_types::error::ErrorDetails;

use crate::models::workbench::{TxAccess, TxState};
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;
use crate::services::nodedb::service::next_id;

/// A failed block refuses every statement except transaction control. This
/// read answers in an open block and is refused in a failed one.
const PROBE: &str = "SHOW transaction_read_only";

/// The open transaction.
pub(super) struct Tx {
    /// Tells this transaction from every other one.
    id: u64,
    /// The database it runs on.
    database: String,
    /// Statements that succeeded since BEGIN.
    statements: u32,
    /// A statement failed the transaction on the server.
    failed: bool,
    /// `None` while a statement runs on it.
    conn: Option<NativeConnection>,
}

impl Tx {
    fn state(&self) -> TxState {
        if self.failed {
            TxState::Failed
        } else {
            TxState::Active {
                access: TxAccess::ReadWrite,
                statements: self.statements,
            }
        }
    }
}

/// Transaction control written as a statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Control {
    Begin,
    Commit,
    Rollback,
}

/// The words of `sql`, upper-cased, as the server's transaction-control
/// tokenizer reads them: comments skipped, a closing `;` allowed. `None`
/// when anything follows the `;` or a token is not a word or a comma.
fn words(sql: &str) -> Option<Vec<String>> {
    let mut out = Vec::new();
    let mut chars = sql.chars().peekable();
    let mut ended = false;
    while let Some(c) = chars.next() {
        if c.is_whitespace() {
            continue;
        }
        if c == '-' && chars.peek() == Some(&'-') {
            chars.by_ref().find(|&n| n == '\n');
            continue;
        }
        if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut depth = 1usize;
            while depth > 0 {
                match chars.next()? {
                    '*' if chars.peek() == Some(&'/') => {
                        chars.next();
                        depth -= 1;
                    }
                    '/' if chars.peek() == Some(&'*') => {
                        chars.next();
                        depth += 1;
                    }
                    _ => {}
                }
            }
            continue;
        }
        if c == ';' {
            ended = true;
            continue;
        }
        if ended {
            return None;
        }
        if c == ',' {
            out.push(",".to_string());
        } else if c.is_alphabetic() || c == '_' {
            let mut word = String::from(c);
            while let Some(&n) = chars
                .peek()
                .filter(|n| n.is_alphanumeric() || **n == '_' || **n == '$')
            {
                word.push(n);
                chars.next();
            }
            out.push(word.to_ascii_uppercase());
        } else {
            return None;
        }
    }
    Some(out)
}

/// The transaction control `sql` spells. COMMIT and ROLLBACK follow the
/// server's grammar exactly, so `COMMIT OFFSETS ...` and `ROLLBACK TO s1`
/// stay statements. Any `BEGIN` or `START TRANSACTION` counts, so no
/// spelling the server accepts can open a transaction on a pooled
/// connection.
fn control_of(sql: &str) -> Option<Control> {
    let words = words(sql)?;
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    let skip_noise = |rest: &[&str]| -> usize {
        usize::from(matches!(rest.first(), Some(&("WORK" | "TRANSACTION"))))
    };
    let ends = |rest: &[&str]| matches!(&rest[skip_noise(rest)..], [] | ["AND", "NO", "CHAIN"]);
    match words.as_slice() {
        ["BEGIN", ..] | ["START", "TRANSACTION", ..] => Some(Control::Begin),
        ["COMMIT" | "END", rest @ ..] if ends(rest) => Some(Control::Commit),
        ["ROLLBACK" | "ABORT", rest @ ..] if ends(rest) => Some(Control::Rollback),
        _ => None,
    }
}

/// Whether a BEGIN asks for a read-only transaction.
fn asks_read_only(sql: &str) -> bool {
    words(sql).is_some_and(|w| w.windows(2).any(|p| p[0] == "READ" && p[1] == "ONLY"))
}

/// The tag of a status row, such as `BEGIN` or `COMMIT`.
fn status_of(result: &QueryResult) -> Option<&str> {
    match (result.columns.as_slice(), result.rows.as_slice()) {
        ([column], [row]) if column == "status" => row.first().and_then(Value::as_str),
        _ => None,
    }
}

/// The connection is gone: the client could not write or read the frame.
fn is_dead(e: &NodeDbError) -> bool {
    matches!(e.details(), ErrorDetails::SyncConnectionFailed)
}

/// The transaction ended with its connection. Nothing it wrote is committed.
fn lost(detail: impl std::fmt::Display) -> StudioError {
    StudioError::Connection(NodeDbError::sync_connection_failed(format!(
        "the transaction ended and its uncommitted statements are gone: {detail}"
    )))
}

fn busy() -> StudioError {
    StudioError::Setup(NodeDbError::bad_request(
        "a statement is still running in the open transaction: wait for it to finish, then try again",
    ))
}

/// NodeDB 0.6.2 stores the access mode of `BEGIN READ ONLY` and still runs
/// writes inside it, then commits them. Studio does not claim a guarantee
/// the server does not give.
pub(super) fn read_only_unsupported() -> StudioError {
    StudioError::Setup(NodeDbError::config(
        "NodeDB 0.6.2 runs and commits writes inside BEGIN READ ONLY, so Studio opens read-write transactions only",
    ))
}

/// Open the connection a transaction pins, as the pool would.
async fn open(config: &PoolConfig) -> Result<NativeConnection, StudioError> {
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

/// The transaction's connection, out of the session while a statement runs
/// on it. Dropped without being returned (a cancelled call), it ends the
/// transaction: the connection closes and the server rolls back.
struct Lease<'a> {
    svc: &'a NodeDbConnectionService,
    id: u64,
    conn: Option<NativeConnection>,
}

impl Lease<'_> {
    /// Put the connection back after `update`, and read the new state. A
    /// transaction that ended meanwhile (a reconnect) takes the connection
    /// with it.
    fn put_back(mut self, update: impl FnOnce(&mut Tx)) -> Result<TxState, StudioError> {
        let conn = self.conn.take();
        let mut session = self.svc.session.borrow_mut();
        match session
            .as_mut()
            .and_then(|s| s.tx.as_mut())
            .filter(|tx| tx.id == self.id)
        {
            Some(tx) => {
                tx.conn = conn;
                update(tx);
                Ok(tx.state())
            }
            None => Err(lost("the session changed while the statement ran")),
        }
    }

    /// End the transaction: close the connection and forget it.
    fn end(mut self) {
        self.conn = None;
        self.svc.forget(self.id);
    }
}

impl Drop for Lease<'_> {
    fn drop(&mut self) {
        if self.conn.take().is_some() {
            self.svc.forget(self.id);
        }
    }
}

/// Where a statement runs.
enum Route<'a> {
    Pool(Rc<NativeClient>),
    Tx(Lease<'a>),
}

impl NodeDbConnectionService {
    /// End transaction `id` if it is still the session's.
    fn forget(&self, id: u64) {
        // `try_` because this runs from `Drop`. Every borrow of the session
        // is short and never held across an await, so it succeeds.
        if let Ok(mut session) = self.session.try_borrow_mut()
            && let Some(s) = session.as_mut()
            && s.tx.as_ref().is_some_and(|tx| tx.id == id)
        {
            s.tx = None;
        }
    }

    /// The session's transaction.
    pub(super) fn tx_state(&self) -> Result<TxState, StudioError> {
        let session = self.session.borrow();
        let session = session.as_ref().ok_or(StudioError::NotConnected)?;
        Ok(session.tx.as_ref().map_or(TxState::Idle, Tx::state))
    }

    /// Where a statement on `database` runs: in the open transaction when it
    /// runs on that database, else on the pool.
    fn route(&self, database: &str) -> Result<Route<'_>, StudioError> {
        {
            let mut session = self.session.borrow_mut();
            let s = session.as_mut().ok_or(StudioError::NotConnected)?;
            let database = s.database_or_default(database);
            if let Some(tx) = s.tx.as_mut() {
                // Run elsewhere, the statement would land outside the
                // transaction the user sees open.
                if tx.database != database {
                    return Err(StudioError::TransactionOpen);
                }
                if tx.failed {
                    return Err(StudioError::Rejected {
                        reason:
                            "the transaction failed: roll it back before running more statements"
                                .to_string(),
                    });
                }
                let conn = tx.conn.take().ok_or_else(busy)?;
                return Ok(Route::Tx(Lease {
                    svc: self,
                    id: tx.id,
                    conn: Some(conn),
                }));
            }
        }
        Ok(Route::Pool(self.client(Some(database))?))
    }

    /// Run one statement on `database` and return the transaction state
    /// after it. Transaction control moves the session's transaction.
    pub(super) async fn execute(
        &self,
        database: &str,
        sql: &str,
        values: &[Value],
    ) -> Result<(QueryResult, TxState), StudioError> {
        match control_of(sql) {
            Some(Control::Begin) => return self.begin_tx(database, sql).await,
            Some(Control::Commit) => return self.end_tx(true).await,
            Some(Control::Rollback) => return self.end_tx(false).await,
            None => {}
        }
        let mut lease = match self.route(database)? {
            Route::Pool(client) => {
                let result = if values.is_empty() {
                    client.query(sql).await?
                } else {
                    client.execute_sql(sql, values).await?
                };
                return Ok((result, TxState::Idle));
            }
            Route::Tx(lease) => lease,
        };
        let Some(conn) = lease.conn.as_mut() else {
            return Err(busy());
        };
        let failure = match conn.execute_sql_with_params(sql, values).await {
            // A COMMIT the classifier missed still ends the transaction.
            Ok(result) if status_of(&result) == Some("COMMIT") => {
                lease.end();
                return Ok((result, TxState::Idle));
            }
            Ok(result) => {
                let state = lease.put_back(|tx| tx.statements += 1)?;
                return Ok((result, state));
            }
            Err(e) => e,
        };
        if is_dead(&failure) {
            lease.end();
            return Err(lost(failure));
        }
        // 0.6.2 fails the transaction on a planning or execution error and
        // keeps it open on a DDL error. Ask the server which one this was.
        let failed = match conn.execute_sql(PROBE).await {
            Ok(_) => false,
            Err(e) if is_dead(&e) => {
                lease.end();
                return Err(lost(e));
            }
            Err(_) => true,
        };
        lease.put_back(|tx| tx.failed = failed)?;
        Err(failure.into())
    }

    /// Open a transaction on `database` by running `sql`, a BEGIN, on a new
    /// connection.
    pub(super) async fn begin_tx(
        &self,
        database: &str,
        sql: &str,
    ) -> Result<(QueryResult, TxState), StudioError> {
        if asks_read_only(sql) {
            return Err(read_only_unsupported());
        }
        let (session_id, database, config) = {
            let session = self.session.borrow();
            let s = session.as_ref().ok_or(StudioError::NotConnected)?;
            if s.tx.is_some() {
                return Err(StudioError::TransactionOpen);
            }
            let database = s.database_or_default(database);
            let config = PoolConfig {
                database: Some(database.clone()),
                ..s.config.clone()
            };
            (s.id, database, config)
        };
        let mut conn = open(&config).await?;
        let result = conn.execute_sql(sql).await?;
        // Anything but the BEGIN tag means the server did not open a block.
        // Dropping `conn` closes it.
        if status_of(&result) != Some("BEGIN") {
            return Err(StudioError::Setup(NodeDbError::bad_request(format!(
                "the server did not open a transaction for `{sql}`"
            ))));
        }
        let mut session = self.session.borrow_mut();
        let s = session
            .as_mut()
            .filter(|s| s.id == session_id)
            .ok_or_else(|| lost("the session changed while the transaction opened"))?;
        // Another BEGIN finished first.
        if s.tx.is_some() {
            return Err(StudioError::TransactionOpen);
        }
        let tx = Tx {
            id: next_id(),
            database,
            statements: 0,
            failed: false,
            conn: Some(conn),
        };
        let state = tx.state();
        s.tx = Some(tx);
        Ok((result, state))
    }

    /// Commit the open transaction, or roll it back. A failed transaction
    /// always rolls back: 0.6.2 answers COMMIT in a failed block by
    /// committing the writes made before the failure.
    pub(super) async fn end_tx(&self, commit: bool) -> Result<(QueryResult, TxState), StudioError> {
        let (mut lease, failed) = {
            let mut session = self.session.borrow_mut();
            let s = session.as_mut().ok_or(StudioError::NotConnected)?;
            let tx = s.tx.as_mut().ok_or(StudioError::NoTransaction)?;
            let conn = tx.conn.take().ok_or_else(busy)?;
            let lease = Lease {
                svc: self,
                id: tx.id,
                conn: Some(conn),
            };
            (lease, tx.failed)
        };
        let sql = if commit && !failed {
            "COMMIT"
        } else {
            "ROLLBACK"
        };
        let out = match lease.conn.as_mut() {
            Some(conn) => conn.execute_sql(sql).await,
            None => return Err(busy()),
        };
        // Whatever the server answered, the transaction is over. A refused
        // COMMIT rolled back on the server, and closing the connection rolls
        // back anything left.
        lease.end();
        match out {
            Ok(result) => Ok((result, TxState::Idle)),
            Err(e) if is_dead(&e) => Err(lost(e)),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begin_in_any_spelling_is_control() {
        for sql in [
            "BEGIN",
            "begin;",
            "Begin Work",
            "START TRANSACTION",
            "start transaction isolation level serializable, read only ;",
            "-- open\nBEGIN /* a /* nested */ note */",
            "BEGIN FOO",
        ] {
            assert_eq!(control_of(sql), Some(Control::Begin), "{sql}");
        }
    }

    #[test]
    fn commit_and_rollback_follow_the_server_grammar() {
        for sql in [
            "COMMIT",
            "commit work",
            "END TRANSACTION;",
            "COMMIT AND NO CHAIN",
        ] {
            assert_eq!(control_of(sql), Some(Control::Commit), "{sql}");
        }
        for sql in ["ROLLBACK", "abort work", "ROLLBACK -- end the block\n"] {
            assert_eq!(control_of(sql), Some(Control::Rollback), "{sql}");
        }
    }

    #[test]
    fn statements_that_only_look_like_control_are_statements() {
        for sql in [
            "SELECT 1",
            "BEGINNING",
            "ROLLBACK TO s1",
            "ROLLBACK TO SAVEPOINT s1",
            "COMMIT AND CHAIN",
            "COMMIT OFFSETS ON s CONSUMER GROUP g",
            "SAVEPOINT s1",
            "BEGIN; SELECT 1",
            "",
        ] {
            assert_eq!(control_of(sql), None, "{sql}");
        }
    }

    #[test]
    fn read_only_is_found_among_the_modes() {
        assert!(asks_read_only("BEGIN READ ONLY"));
        assert!(asks_read_only(
            "start transaction isolation level serializable, read only;"
        ));
        assert!(!asks_read_only("BEGIN READ WRITE"));
        assert!(!asks_read_only("BEGIN"));
    }

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

    mod live {
        use crate::models::console::{ConsoleOutput, Protocol};
        use crate::models::workbench::{QueryRequest, TxAccess, TxState};
        use crate::services::connection_service::ConnectionService;
        use crate::services::error::StudioError;
        use crate::services::nodedb::NodeDbConnectionService;
        use crate::services::nodedb::service::{live, to_table};
        use crate::services::workbench_data::WorkbenchData;
        use crate::state::connections_registry::Credentials;

        fn q(sql: &str) -> QueryRequest {
            QueryRequest {
                sql: sql.into(),
                params: Vec::new(),
                database: String::new(),
                page: 0,
                page_size: 50,
            }
        }

        fn active(statements: u32) -> TxState {
            TxState::Active {
                access: TxAccess::ReadWrite,
                statements,
            }
        }

        /// An empty `table`. It is never dropped, so a concurrent catalog
        /// read never meets a collection that vanished under it.
        async fn seed(svc: &NodeDbConnectionService, table: &str) {
            let c = svc.client(None).expect("a client");
            for sql in [
                format!("CREATE COLLECTION IF NOT EXISTS {table}"),
                format!("DELETE FROM {table}"),
            ] {
                c.query(&sql).await.expect("seed runs");
            }
        }

        /// The ids of `table` as the pool reads them: outside any
        /// transaction.
        async fn outside(svc: &NodeDbConnectionService, table: &str) -> Vec<String> {
            let c = svc.client(None).expect("a client");
            let result = c
                .query(&format!("SELECT id FROM {table}"))
                .await
                .expect("reads");
            let mut ids: Vec<String> = to_table(&result)
                .rows
                .into_iter()
                .filter_map(|r| r.into_iter().next())
                .collect();
            ids.sort();
            ids
        }

        #[tokio::test]
        #[ignore = "needs a live NodeDB 0.6.2 server: see `live`"]
        async fn live_tx_uncommitted_writes_stay_inside_until_rollback() {
            let svc = live().await;
            let t = "studio_probe_w_tx_rb";
            seed(&svc, t).await;
            assert_eq!(svc.transaction().await.expect("state"), TxState::Idle);
            let begun = svc
                .begin_transaction(TxAccess::ReadWrite)
                .await
                .expect("begins");
            assert_eq!(begun, active(0));
            let insert = format!("INSERT INTO {t} (id, n) VALUES ('a', 1)");
            let run = svc.run_query(&q(&insert)).await.expect("inserts");
            assert_eq!(run.tx, active(1));
            let inside = svc
                .run_query(&q(&format!("SELECT id FROM {t}")))
                .await
                .expect("selects");
            assert_eq!(inside.result.total_rows, 1, "the write is visible inside");
            assert_eq!(inside.tx, active(2));
            assert!(outside(&svc, t).await.is_empty(), "and nowhere else");
            assert_eq!(
                svc.rollback_transaction().await.expect("rolls back"),
                TxState::Idle
            );
            assert_eq!(svc.transaction().await.expect("state"), TxState::Idle);
            assert!(outside(&svc, t).await.is_empty(), "rollback discards it");
            assert!(matches!(
                svc.rollback_transaction().await,
                Err(StudioError::NoTransaction)
            ));
        }

        #[tokio::test]
        #[ignore = "needs a live NodeDB 0.6.2 server: see `live`"]
        async fn live_tx_commit_persists() {
            let svc = live().await;
            let t = "studio_probe_w_tx_commit";
            seed(&svc, t).await;
            svc.begin_transaction(TxAccess::ReadWrite)
                .await
                .expect("begins");
            for id in ["a", "b"] {
                svc.run_query(&q(&format!("INSERT INTO {t} (id, n) VALUES ('{id}', 1)")))
                    .await
                    .expect("inserts");
            }
            assert!(outside(&svc, t).await.is_empty());
            assert_eq!(
                svc.commit_transaction().await.expect("commits"),
                TxState::Idle
            );
            assert_eq!(outside(&svc, t).await, ["a", "b"]);
            assert!(matches!(
                svc.commit_transaction().await,
                Err(StudioError::NoTransaction)
            ));
        }

        #[tokio::test]
        #[ignore = "needs a live NodeDB 0.6.2 server: see `live`"]
        async fn live_tx_statement_error_fails_until_rollback() {
            let svc = live().await;
            let t = "studio_probe_w_tx_fail";
            seed(&svc, t).await;
            svc.begin_transaction(TxAccess::ReadWrite)
                .await
                .expect("begins");
            svc.run_query(&q(&format!("INSERT INTO {t} (id, n) VALUES ('a', 1)")))
                .await
                .expect("inserts");
            let missing = svc
                .run_query(&q("SELECT * FROM studio_probe_w_missing"))
                .await
                .expect_err("no such collection");
            assert!(matches!(missing, StudioError::NotFound(_)), "{missing:?}");
            assert_eq!(svc.transaction().await.expect("state"), TxState::Failed);
            assert!(matches!(
                svc.run_query(&q("SELECT 1")).await,
                Err(StudioError::Rejected { .. })
            ));
            assert!(matches!(
                svc.begin_transaction(TxAccess::ReadWrite).await,
                Err(StudioError::TransactionOpen)
            ));
            // 0.6.2 would commit the INSERT made before the failure. Studio
            // rolls back instead, as the seam promises.
            assert_eq!(svc.commit_transaction().await.expect("ends"), TxState::Idle);
            assert!(outside(&svc, t).await.is_empty());
            // A rollback leaves a failed transaction too.
            svc.begin_transaction(TxAccess::ReadWrite)
                .await
                .expect("begins");
            assert!(svc.run_query(&q("SELEC 1")).await.is_err());
            assert_eq!(svc.transaction().await.expect("state"), TxState::Failed);
            assert_eq!(
                svc.rollback_transaction().await.expect("rolls back"),
                TxState::Idle
            );
            assert!(svc.run_query(&q("SELECT 1")).await.is_ok());
        }

        #[tokio::test]
        #[ignore = "needs a live NodeDB 0.6.2 server: see `live`"]
        async fn live_tx_ddl_error_keeps_the_transaction_open() {
            let svc = live().await;
            let t = "studio_probe_w_tx_ddl";
            seed(&svc, t).await;
            svc.begin_transaction(TxAccess::ReadWrite)
                .await
                .expect("begins");
            // 0.6.2 refuses the DDL and keeps the block open: Studio reports
            // what the server holds.
            assert!(
                svc.run_query(&q(&format!("CREATE COLLECTION {t}")))
                    .await
                    .is_err()
            );
            assert_eq!(svc.transaction().await.expect("state"), active(0));
            // DDL inside a transaction is buffered until COMMIT.
            let inner = "studio_probe_w_tx_ddl_inner";
            svc.run_query(&q(&format!("CREATE COLLECTION {inner}")))
                .await
                .expect("creates inside");
            svc.run_query(&q(&format!("INSERT INTO {inner} (id) VALUES ('a')")))
                .await
                .expect("writes inside");
            svc.rollback_transaction().await.expect("rolls back");
            let gone = svc
                .client(None)
                .expect("a client")
                .query(&format!("SELECT id FROM {inner}"))
                .await;
            assert!(gone.is_err(), "the rollback dropped the new collection");
        }

        #[tokio::test]
        #[ignore = "needs a live NodeDB 0.6.2 server: see `live`"]
        async fn live_tx_statements_share_one_connection() {
            let svc = live().await;
            let t = "studio_probe_w_tx_one";
            seed(&svc, t).await;
            svc.begin_transaction(TxAccess::ReadWrite)
                .await
                .expect("begins");
            // More statements than the pool has connections. Each sees every
            // earlier uncommitted row, which only the same connection can.
            for i in 0..6 {
                let run = svc
                    .run_query(&q(&format!("INSERT INTO {t} (id, n) VALUES ('r{i}', {i})")))
                    .await
                    .expect("inserts");
                assert_eq!(run.tx, active(2 * i + 1));
                let seen = svc
                    .run_query(&q(&format!("SELECT id FROM {t}")))
                    .await
                    .expect("selects");
                assert_eq!(seen.result.total_rows, i as usize + 1);
            }
            assert!(outside(&svc, t).await.is_empty());
            // While a statement holds the connection, a second one is refused
            // and a read outside the transaction still runs.
            let select = q(&format!("SELECT id FROM {t}"));
            let (first, second, dbs) = tokio::join!(
                svc.run_query(&select),
                svc.run_query(&select),
                svc.databases()
            );
            assert!(first.is_ok());
            assert!(matches!(second, Err(StudioError::Setup(_))), "{second:?}");
            assert!(dbs.is_ok());
            assert_eq!(svc.transaction().await.expect("state"), active(13));
            svc.rollback_transaction().await.expect("rolls back");
            assert!(outside(&svc, t).await.is_empty());
        }

        #[tokio::test]
        #[ignore = "needs a live NodeDB 0.6.2 server: see `live`"]
        async fn live_tx_read_only_is_refused() {
            let svc = live().await;
            assert!(matches!(
                svc.begin_transaction(TxAccess::ReadOnly).await,
                Err(StudioError::Setup(_))
            ));
            assert!(matches!(
                svc.console_run(Protocol::Sql, "", "START TRANSACTION READ ONLY;")
                    .await,
                Err(StudioError::Setup(_))
            ));
            assert_eq!(svc.transaction().await.expect("state"), TxState::Idle);
        }

        #[tokio::test]
        #[ignore = "needs a live NodeDB 0.6.2 server: see `live`"]
        async fn live_tx_reconnect_ends_the_transaction() {
            let svc = live().await;
            let t = "studio_probe_w_tx_reconnect";
            seed(&svc, t).await;
            svc.begin_transaction(TxAccess::ReadWrite)
                .await
                .expect("begins");
            svc.run_query(&q(&format!("INSERT INTO {t} (id, n) VALUES ('a', 1)")))
                .await
                .expect("inserts");
            let creds = Credentials {
                username: std::env::var("NODEDB_PROBE_USER").expect("set NODEDB_PROBE_USER"),
                password: None,
            };
            svc.connect("probe", &creds).await.expect("reconnects");
            assert_eq!(svc.transaction().await.expect("state"), TxState::Idle);
            assert!(outside(&svc, t).await.is_empty());
            assert!(svc.begin_transaction(TxAccess::ReadWrite).await.is_ok());
            svc.rollback_transaction().await.expect("rolls back");
        }

        #[tokio::test]
        #[ignore = "needs a live NodeDB 0.6.2 server: see `live`"]
        async fn live_console_and_query_share_one_transaction() {
            let svc = live().await;
            let t = "studio_probe_w_tx_console";
            seed(&svc, t).await;
            let begun = svc
                .console_run(Protocol::Sql, "", "begin;")
                .await
                .expect("begins");
            assert_eq!(begun.tx, active(0));
            assert_eq!(svc.transaction().await.expect("state"), begun.tx);
            let run = svc
                .run_query(&q(&format!("INSERT INTO {t} (id, n) VALUES ('a', 1)")))
                .await
                .expect("inserts");
            assert_eq!(run.tx, active(1));
            let seen = svc
                .console_run(Protocol::Sql, "", &format!("SELECT id FROM {t};"))
                .await
                .expect("selects");
            assert_eq!(
                seen.output,
                ConsoleOutput::Table {
                    columns: vec!["id".into()],
                    rows: vec![vec!["a".into()]],
                }
            );
            assert_eq!(seen.tx, active(2));
            // BEGIN typed into the Query page opens nothing new.
            assert!(matches!(
                svc.run_query(&q("BEGIN")).await,
                Err(StudioError::TransactionOpen)
            ));
            let done = svc
                .console_run(Protocol::Sql, "", "ROLLBACK -- undo\n")
                .await
                .expect("rolls back");
            assert_eq!(done.tx, TxState::Idle);
            assert!(outside(&svc, t).await.is_empty());
            assert!(matches!(
                svc.console_run(Protocol::Sql, "", "COMMIT").await,
                Err(StudioError::NoTransaction)
            ));
        }
    }
}
