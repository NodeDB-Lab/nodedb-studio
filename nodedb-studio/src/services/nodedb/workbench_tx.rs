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
//!
//! Every call on the pinned connection has a time limit. A call that runs
//! past it, or a connection that dies, ends the transaction: Studio closes
//! the connection and the server rolls back.

use std::rc::Rc;
use std::time::{Duration, Instant};

use nodedb_client::native::connection::NativeConnection;
use nodedb_client::native::pool::PoolConfig;
use nodedb_client::{NativeClient, NodeDb, NodeDbError, QueryResult};
use nodedb_types::Value;

use crate::models::workbench::{TxAccess, TxState};
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;
use crate::services::nodedb::service::next_id;
use crate::services::nodedb::workbench_conn::{
    CONTROL_LIMIT, Gone, STATEMENT_LIMIT, bounded, ended, failed_on, lost, open, status_of,
};
use crate::services::nodedb::workbench_sql::{Control, asks_read_only, control_of, resets_session};

/// `tx_checked` asks the server after the connection sat idle this long.
/// 0.6.2 closes a native session idle for `idle_timeout_secs` (3600 s by
/// default) and rolls its transaction back.
const IDLE_PROBE_AFTER: Duration = Duration::from_secs(30);

/// Shown with a COMMIT that found the transaction failed.
pub(super) const ROLLED_BACK: &str = "The transaction had failed, so Studio rolled it back instead of committing. Nothing was committed.";

/// The open transaction.
pub(super) struct Tx {
    /// Tells this transaction from every other one.
    id: u64,
    /// The database it runs on.
    database: String,
    /// Statements that succeeded since BEGIN and that no ROLLBACK TO undid.
    statements: u32,
    /// Each open savepoint, oldest first, with the statement count right
    /// after it.
    savepoints: Vec<(String, u32)>,
    /// A statement failed the transaction on the server.
    failed: bool,
    /// When the last call on the connection finished.
    used: Instant,
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

    /// Count a statement that succeeded. Savepoints follow the server: a
    /// RELEASE drops the savepoint and every later one, and a ROLLBACK TO
    /// keeps the savepoint and undoes every statement after it.
    fn ran(&mut self, control: Option<&Control>) {
        let at = |tx: &Self, name: &str| tx.savepoints.iter().rposition(|(n, _)| n == name);
        match control {
            Some(Control::Savepoint(name)) => {
                self.statements += 1;
                self.savepoints.push((name.clone(), self.statements));
            }
            Some(Control::Release(name)) => {
                if let Some(i) = at(self, name) {
                    self.savepoints.truncate(i);
                }
                self.statements += 1;
            }
            Some(Control::RollbackTo(name)) => {
                if let Some(i) = at(self, name) {
                    self.statements = self.savepoints[i].1;
                    self.savepoints.truncate(i + 1);
                }
                self.statements += 1;
            }
            Some(Control::Begin | Control::Commit | Control::Rollback) | None => {
                self.statements += 1;
            }
        }
    }
}

/// A statement's result and the transaction state after it.
pub(super) struct Ran {
    pub result: QueryResult,
    pub tx: TxState,
    /// What the user must read with the result.
    pub warning: Option<&'static str>,
}

impl Ran {
    fn new(result: QueryResult, tx: TxState) -> Self {
        Self {
            result,
            tx,
            warning: None,
        }
    }
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
                tx.used = Instant::now();
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

    /// The session's transaction, checked with the server when its
    /// connection sat idle. A connection the server closed ends the
    /// transaction: this call reports the loss, and later calls read Idle.
    pub(super) async fn tx_checked(&self) -> Result<TxState, StudioError> {
        let mut lease = {
            let mut session = self.session.borrow_mut();
            let s = session.as_mut().ok_or(StudioError::NotConnected)?;
            let Some(tx) = s.tx.as_mut() else {
                return Ok(TxState::Idle);
            };
            if tx.used.elapsed() < IDLE_PROBE_AFTER {
                return Ok(tx.state());
            }
            // A statement runs on it, so it is in use.
            let Some(conn) = tx.conn.take() else {
                return Ok(tx.state());
            };
            Lease {
                svc: self,
                id: tx.id,
                conn: Some(conn),
            }
        };
        let Some(conn) = lease.conn.as_mut() else {
            return Err(busy());
        };
        match failed_on(conn).await {
            Ok(failed) => lease.put_back(|tx| tx.failed = failed),
            Err(Gone(detail)) => {
                lease.end();
                Err(lost(detail))
            }
        }
    }

    /// Where statement `sql` on `database` runs: in the open transaction
    /// when it runs on that database, else on the pool.
    fn route(
        &self,
        database: &str,
        sql: &str,
        control: Option<&Control>,
    ) -> Result<Route<'_>, StudioError> {
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
                // 0.6.2 drops the transaction on DISCARD ALL, then commits
                // each later statement at once while Studio shows it open.
                if resets_session(sql) {
                    return Err(StudioError::TransactionOpen);
                }
                // The server runs ROLLBACK TO in a failed block: it rewinds
                // to the savepoint and clears the failure.
                if tx.failed && !matches!(control, Some(Control::RollbackTo(_))) {
                    return Err(StudioError::Rejected {
                        reason: "the transaction failed: roll it back, or back to a savepoint, before running more statements"
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
    ) -> Result<Ran, StudioError> {
        let control = control_of(sql);
        match control {
            Some(Control::Begin) => return self.begin_tx(database, sql).await,
            Some(Control::Commit) => return self.end_tx(true).await,
            Some(Control::Rollback) => return self.end_tx(false).await,
            Some(Control::Savepoint(_) | Control::Release(_) | Control::RollbackTo(_)) | None => {}
        }
        let mut lease = match self.route(database, sql, control.as_ref())? {
            Route::Pool(client) => {
                let result = if values.is_empty() {
                    client.query(sql).await?
                } else {
                    client.execute_sql(sql, values).await?
                };
                return Ok(Ran::new(result, TxState::Idle));
            }
            Route::Tx(lease) => lease,
        };
        let Some(conn) = lease.conn.as_mut() else {
            return Err(busy());
        };
        let answer = match bounded(STATEMENT_LIMIT, conn.execute_sql_with_params(sql, values)).await
        {
            Err(Gone(detail)) => {
                lease.end();
                return Err(lost(detail));
            }
            // A COMMIT the classifier missed still ends the transaction.
            Ok(Ok(result)) if status_of(&result) == Some("COMMIT") => {
                lease.end();
                return Ok(Ran::new(result, TxState::Idle));
            }
            Ok(answer) => answer,
        };
        // 0.6.2 fails the transaction on a planning or execution error, keeps
        // it open on a DDL error, and clears a failure on ROLLBACK TO. Ask
        // the server which one holds.
        let ask = answer.is_err() || matches!(control, Some(Control::RollbackTo(_)));
        let failed = if ask {
            match failed_on(conn).await {
                Ok(failed) => failed,
                Err(Gone(detail)) => {
                    lease.end();
                    return Err(lost(detail));
                }
            }
        } else {
            false
        };
        match answer {
            Ok(result) => {
                let tx = lease.put_back(|tx| {
                    tx.ran(control.as_ref());
                    tx.failed = failed;
                })?;
                Ok(Ran::new(result, tx))
            }
            Err(e) => {
                lease.put_back(|tx| tx.failed = failed)?;
                Err(e.into())
            }
        }
    }

    /// Open a transaction on `database` by running `sql`, a BEGIN, on a new
    /// connection.
    pub(super) async fn begin_tx(&self, database: &str, sql: &str) -> Result<Ran, StudioError> {
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
        // Dropping `conn` on any way out closes it, and the server rolls
        // back whatever it opened.
        let result = match bounded(CONTROL_LIMIT, conn.execute_sql(sql)).await {
            Ok(answer) => answer?,
            Err(Gone(detail)) => {
                return Err(StudioError::Connection(
                    NodeDbError::sync_connection_failed(detail),
                ));
            }
        };
        // Anything but the BEGIN tag means the server did not open a block.
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
            savepoints: Vec::new(),
            failed: false,
            used: Instant::now(),
            conn: Some(conn),
        };
        let state = tx.state();
        s.tx = Some(tx);
        Ok(Ran::new(result, state))
    }

    /// Commit the open transaction, or roll it back. A failed transaction
    /// always rolls back, with a warning: 0.6.2 answers COMMIT in a failed
    /// block by committing the writes made before the failure.
    pub(super) async fn end_tx(&self, commit: bool) -> Result<Ran, StudioError> {
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
        let committing = commit && !failed;
        let sql = if committing { "COMMIT" } else { "ROLLBACK" };
        let answer = match lease.conn.as_mut() {
            Some(conn) => bounded(CONTROL_LIMIT, conn.execute_sql(sql)).await,
            None => return Err(busy()),
        };
        // Whatever the server answered, the transaction is over.
        lease.end();
        Ok(Ran {
            result: ended(committing, answer)?,
            tx: TxState::Idle,
            warning: (commit && failed).then_some(ROLLED_BACK),
        })
    }
}

#[cfg(test)]
mod tests {
    use nodedb_types::protocol::AuthMethod as WireAuth;

    use super::*;
    use crate::services::nodedb::service::Session;

    fn tx(used: Instant) -> Tx {
        Tx {
            id: next_id(),
            database: "default".into(),
            statements: 0,
            savepoints: Vec::new(),
            failed: false,
            used,
            conn: None,
        }
    }

    #[test]
    fn rollback_to_a_savepoint_undoes_the_count_after_it() {
        let mut tx = tx(Instant::now());
        let sp = |n: &str| Control::Savepoint(n.into());
        let to = |n: &str| Control::RollbackTo(n.into());
        tx.ran(None);
        tx.ran(Some(&sp("a")));
        tx.ran(None);
        tx.ran(Some(&sp("b")));
        tx.ran(None);
        tx.ran(Some(&to("b")));
        assert_eq!(tx.statements, 5, "four up to b, and the ROLLBACK TO");
        assert_eq!(tx.savepoints.len(), 2, "b stays");
        tx.ran(Some(&to("a")));
        assert_eq!(tx.statements, 3);
        assert_eq!(tx.savepoints, [("a".to_string(), 2)], "b is gone");
        tx.ran(Some(&Control::Release("a".into())));
        assert_eq!(tx.statements, 4);
        assert!(tx.savepoints.is_empty());
        // A name Studio does not hold changes only the count.
        tx.ran(Some(&to("x")));
        assert_eq!(tx.statements, 5);
    }

    #[test]
    fn a_reused_savepoint_name_means_the_latest() {
        let mut tx = tx(Instant::now());
        tx.ran(Some(&Control::Savepoint("a".into())));
        tx.ran(None);
        tx.ran(Some(&Control::Savepoint("a".into())));
        tx.ran(None);
        tx.ran(Some(&Control::RollbackTo("a".into())));
        assert_eq!(tx.statements, 4);
        assert_eq!(tx.savepoints.len(), 2);
    }

    /// A service whose session holds `tx` and never reached a server.
    fn holding(tx: Tx) -> NodeDbConnectionService {
        let config = PoolConfig::new(
            "127.0.0.1:1",
            WireAuth::Trust {
                username: "alice".into(),
            },
        );
        let svc = NodeDbConnectionService::default();
        let mut session = Session::new(config, "default".into(), "0.6.2".into(), "alice");
        session.tx = Some(tx);
        *svc.session.borrow_mut() = Some(session);
        svc
    }

    #[tokio::test]
    async fn the_state_read_asks_the_server_only_after_idle() {
        let active = TxState::Active {
            access: TxAccess::ReadWrite,
            statements: 0,
        };
        // Used just now: no probe, so no network.
        let svc = holding(tx(Instant::now()));
        assert_eq!(svc.tx_checked().await.expect("reads"), active);
        // Idle, but a statement holds the connection: it is in use.
        let long_ago = Instant::now()
            .checked_sub(IDLE_PROBE_AFTER * 2)
            .expect("the clock runs long enough");
        let svc = holding(tx(long_ago));
        assert_eq!(svc.tx_checked().await.expect("reads"), active);
    }

    mod live {
        use std::time::{Duration, Instant};

        use super::super::IDLE_PROBE_AFTER;
        use crate::models::console::{ConsoleOutput, HistoryQuery, HistoryWindow, Protocol};
        use crate::models::saved_query::SaveQuery;
        use crate::models::workbench::{MessageLevel, QueryRequest, TxAccess, TxState};
        use crate::services::connection_service::ConnectionService;
        use crate::services::error::StudioError;
        use crate::services::nodedb::NodeDbConnectionService;
        use crate::services::nodedb::service::{live, to_table};
        use crate::services::nodedb::workbench_conn::is_dead;
        use crate::services::workbench_data::WorkbenchData;
        use crate::state::connections_registry::Credentials;

        /// Kill the transaction's connection, as a server that closed it
        /// leaves it, and make it look idle. A frame over the server's
        /// 16 MiB limit makes the server answer an error and close the
        /// connection.
        async fn kill_pinned(svc: &NodeDbConnectionService) {
            let mut conn = svc
                .session
                .borrow_mut()
                .as_mut()
                .and_then(|s| s.tx.as_mut())
                .and_then(|tx| tx.conn.take())
                .expect("a pinned connection");
            let huge = format!("SELECT '{}'", "x".repeat(17 * 1024 * 1024));
            let out = tokio::time::timeout(Duration::from_secs(30), conn.execute_sql(&huge))
                .await
                .expect("the server answers in time");
            assert!(
                out.as_ref().is_err_and(is_dead),
                "the connection is dead: {out:?}"
            );
            let mut session = svc.session.borrow_mut();
            let tx = session
                .as_mut()
                .and_then(|s| s.tx.as_mut())
                .expect("still open in Studio");
            tx.conn = Some(conn);
            tx.used = Instant::now()
                .checked_sub(IDLE_PROBE_AFTER * 2)
                .expect("the clock runs long enough");
        }

        fn unknown_outcome(e: &StudioError) -> bool {
            matches!(e, StudioError::Connection(_))
                && !e.is_retriable()
                && e.to_string().contains("can have committed")
        }

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
            // Studio's own records outlive the session.
            let saved = svc
                .save_query(&SaveQuery {
                    id: None,
                    name: "studio_probe_w_saved".into(),
                    folder: None,
                    database: "default".into(),
                    sql: "SELECT 1".into(),
                    params: Vec::new(),
                    tags: Vec::new(),
                })
                .await
                .expect("saves");
            svc.console_run(Protocol::Sql, "", "SELECT 2")
                .await
                .expect("runs");
            let creds = Credentials {
                username: std::env::var("NODEDB_PROBE_USER").expect("set NODEDB_PROBE_USER"),
                password: None,
            };
            svc.connect("probe", &creds).await.expect("reconnects");
            assert_eq!(svc.transaction().await.expect("state"), TxState::Idle);
            assert!(outside(&svc, t).await.is_empty());
            let library = svc.query_library().await.expect("reads");
            assert_eq!(library.queries, std::slice::from_ref(&saved));
            assert_eq!(saved.owner, creds.username);
            let history = svc
                .console_history(&HistoryQuery {
                    protocol: None,
                    window: HistoryWindow::AllTime,
                })
                .await
                .expect("reads");
            assert!(history.iter().any(|h| h.text == "SELECT 2"));
            assert!(svc.begin_transaction(TxAccess::ReadWrite).await.is_ok());
            svc.rollback_transaction().await.expect("rolls back");
        }

        #[tokio::test]
        #[ignore = "needs a live NodeDB 0.6.2 server: see `live`"]
        async fn live_tx_discard_all_is_refused_inside_a_transaction() {
            let svc = live().await;
            let t = "studio_probe_w_tx_discard";
            seed(&svc, t).await;
            svc.begin_transaction(TxAccess::ReadWrite)
                .await
                .expect("begins");
            svc.run_query(&q(&format!("INSERT INTO {t} (id, n) VALUES ('a', 1)")))
                .await
                .expect("inserts");
            // 0.6.2 would drop the transaction, then commit each later
            // statement at once while Studio showed it open.
            for sql in ["DISCARD ALL", "discard all;", "/* reset */ DISCARD ALL"] {
                let refused = svc.run_query(&q(sql)).await;
                assert!(
                    matches!(refused, Err(StudioError::TransactionOpen)),
                    "{sql}: {refused:?}"
                );
            }
            assert!(matches!(
                svc.console_run(Protocol::Sql, "", "DISCARD ALL").await,
                Err(StudioError::TransactionOpen)
            ));
            assert_eq!(svc.transaction().await.expect("state"), active(1));
            svc.run_query(&q(&format!("INSERT INTO {t} (id, n) VALUES ('b', 1)")))
                .await
                .expect("inserts");
            svc.rollback_transaction().await.expect("rolls back");
            assert!(outside(&svc, t).await.is_empty(), "nothing escaped");
            // Outside a transaction it runs on the pool.
            assert!(svc.run_query(&q("DISCARD ALL")).await.is_ok());
        }

        #[tokio::test]
        #[ignore = "needs a live NodeDB 0.6.2 server: see `live`"]
        async fn live_tx_rollback_to_a_savepoint_clears_a_failure() {
            let svc = live().await;
            let t = "studio_probe_w_tx_savepoint";
            seed(&svc, t).await;
            let insert = |id: &str| q(&format!("INSERT INTO {t} (id, n) VALUES ('{id}', 1)"));
            svc.begin_transaction(TxAccess::ReadWrite)
                .await
                .expect("begins");
            svc.run_query(&insert("a")).await.expect("inserts");
            let sp = svc.run_query(&q("SAVEPOINT s")).await.expect("marks");
            assert_eq!(sp.tx, active(2));
            svc.run_query(&insert("b")).await.expect("inserts");
            assert!(
                svc.run_query(&q("SELECT * FROM studio_probe_w_missing"))
                    .await
                    .is_err()
            );
            assert_eq!(svc.transaction().await.expect("state"), TxState::Failed);
            // The server runs ROLLBACK TO in a failed block and clears it.
            let back = svc
                .run_query(&q("rollback to savepoint s"))
                .await
                .expect("rewinds");
            assert_eq!(
                back.tx,
                active(3),
                "the two statements before s, and this one"
            );
            let seen = svc
                .run_query(&q(&format!("SELECT id FROM {t}")))
                .await
                .expect("selects");
            assert_eq!(seen.result.total_rows, 1, "b is gone, a stays");
            // A quoted name keeps its case, as on the server.
            svc.run_query(&q(r#"SAVEPOINT "Q""#)).await.expect("marks");
            svc.run_query(&insert("c")).await.expect("inserts");
            let back = svc
                .console_run(Protocol::Sql, "", r#"ROLLBACK TO "Q";"#)
                .await
                .expect("rewinds");
            assert_eq!(back.tx, active(6));
            // A savepoint that does not exist fails the statement only.
            assert!(svc.run_query(&q("ROLLBACK TO nope")).await.is_err());
            assert_eq!(svc.transaction().await.expect("state"), active(6));
            svc.run_query(&q("RELEASE SAVEPOINT s"))
                .await
                .expect("releases");
            svc.commit_transaction().await.expect("commits");
            assert_eq!(outside(&svc, t).await, ["a"]);
        }

        #[tokio::test]
        #[ignore = "needs a live NodeDB 0.6.2 server: see `live`"]
        async fn live_tx_a_typed_commit_on_a_failed_transaction_warns() {
            let svc = live().await;
            let t = "studio_probe_w_tx_warn";
            seed(&svc, t).await;
            let fail = q("SELECT * FROM studio_probe_w_missing");
            svc.begin_transaction(TxAccess::ReadWrite)
                .await
                .expect("begins");
            svc.run_query(&q(&format!("INSERT INTO {t} (id, n) VALUES ('a', 1)")))
                .await
                .expect("inserts");
            assert!(svc.run_query(&fail).await.is_err());
            let run = svc.run_query(&q("commit")).await.expect("ends");
            assert_eq!(run.tx, TxState::Idle);
            assert!(
                run.messages
                    .iter()
                    .any(|m| m.level == MessageLevel::Warning && m.text.contains("rolled it back")),
                "{:?}",
                run.messages
            );
            svc.console_run(Protocol::Sql, "", "BEGIN")
                .await
                .expect("begins");
            svc.run_query(&q(&format!("INSERT INTO {t} (id, n) VALUES ('b', 1)")))
                .await
                .expect("inserts");
            assert!(svc.run_query(&fail).await.is_err());
            let reply = svc
                .console_run(Protocol::Sql, "", "END;")
                .await
                .expect("ends");
            assert_eq!(reply.tx, TxState::Idle);
            let ConsoleOutput::Text(lines) = reply.output else {
                panic!("a warning line, got {:?}", reply.output);
            };
            assert!(lines.iter().any(|l| l.contains("Nothing was committed")));
            assert!(outside(&svc, t).await.is_empty());
        }

        #[tokio::test]
        #[ignore = "needs a live NodeDB 0.6.2 server: see `live`"]
        async fn live_tx_a_lost_connection_ends_the_transaction() {
            let svc = live().await;
            let t = "studio_probe_w_tx_lost";
            seed(&svc, t).await;
            let insert = |id: &str| q(&format!("INSERT INTO {t} (id, n) VALUES ('{id}', 1)"));
            svc.begin_transaction(TxAccess::ReadWrite)
                .await
                .expect("begins");
            svc.run_query(&insert("a")).await.expect("inserts");
            kill_pinned(&svc).await;
            // The badge read finds the loss and reports it once.
            let lost = svc.transaction().await.expect_err("the loss is reported");
            assert!(matches!(lost, StudioError::Connection(_)), "{lost:?}");
            assert_eq!(svc.transaction().await.expect("state"), TxState::Idle);
            assert!(outside(&svc, t).await.is_empty());
            // COMMIT with no answer: the outcome is unknown, and no Retry.
            svc.begin_transaction(TxAccess::ReadWrite)
                .await
                .expect("begins");
            svc.run_query(&insert("b")).await.expect("inserts");
            kill_pinned(&svc).await;
            let unknown = svc.commit_transaction().await.expect_err("no answer");
            assert!(unknown_outcome(&unknown), "{unknown:?}");
            assert_eq!(svc.transaction().await.expect("state"), TxState::Idle);
            svc.console_run(Protocol::Sql, "", "BEGIN")
                .await
                .expect("begins");
            kill_pinned(&svc).await;
            let unknown = svc
                .console_run(Protocol::Sql, "", "COMMIT;")
                .await
                .expect_err("no answer");
            assert!(unknown_outcome(&unknown), "{unknown:?}");
            // ROLLBACK with no answer: the server rolled back on close.
            svc.begin_transaction(TxAccess::ReadWrite)
                .await
                .expect("begins");
            kill_pinned(&svc).await;
            let gone = svc.rollback_transaction().await.expect_err("no answer");
            assert!(gone.to_string().contains("uncommitted statements are gone"));
            assert!(outside(&svc, t).await.is_empty());
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
