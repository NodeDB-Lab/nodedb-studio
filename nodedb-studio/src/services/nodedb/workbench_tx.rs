//! The session's statements and its transaction.
//!
//! Every statement the user types runs on the statement connection of its
//! database (see `workbench_conn`). BEGIN opens the transaction on that same
//! connection, so the settings the user made before BEGIN hold inside it,
//! and one connection carries all of the user's server state. A COMMIT or
//! ROLLBACK the server carries out leaves the connection open for the next
//! statement.
//!
//! Every call has a time limit. A call that runs past it, or a connection
//! that dies, closes the connection: the server rolls back an open
//! transaction, and the next statement opens a new connection.

use nodedb_client::{NodeDbError, QueryResult};
use nodedb_types::Value;

use crate::models::workbench::{TxAccess, TxState};
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;
use crate::services::nodedb::service::next_id;
use crate::services::nodedb::workbench_conn::{
    CONTROL_LIMIT, Gone, SETTINGS_RESET, STATEMENT_LIMIT, bounded, ended, failed_on, lost,
    no_answer, status_of,
};
use crate::services::nodedb::workbench_sql::{Control, asks_read_only, control_of, resets_session};

/// Shown with a COMMIT that found the transaction failed.
pub(super) const ROLLED_BACK: &str = "The transaction had failed, so Studio rolled it back instead of committing. Nothing was committed.";

/// The open transaction. It runs on its database's statement connection.
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
}

impl Tx {
    pub(super) fn id(&self) -> u64 {
        self.id
    }

    pub(super) fn database(&self) -> &str {
        &self.database
    }

    pub(super) fn state(&self) -> TxState {
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
    pub warnings: Vec<&'static str>,
}

impl Ran {
    /// `reset`: the statement ran on a connection that replaces one that
    /// closed.
    fn new(result: QueryResult, tx: TxState, reset: bool) -> Self {
        Self {
            result,
            tx,
            warnings: reset.then_some(SETTINGS_RESET).into_iter().collect(),
        }
    }
}

/// NodeDB 0.6.2 stores the access mode of `BEGIN READ ONLY` and still runs
/// writes inside it, then commits them. Studio does not claim a guarantee
/// the server does not give.
pub(super) fn read_only_unsupported() -> StudioError {
    StudioError::Setup(NodeDbError::config(
        "NodeDB 0.6.2 runs and commits writes inside BEGIN READ ONLY, so Studio opens read-write transactions only",
    ))
}

/// Whether statement `sql` can run against `database` while `tx` is open.
fn may_run(
    sql: &str,
    control: Option<&Control>,
    database: &str,
    tx: Option<&Tx>,
) -> Result<(), StudioError> {
    let Some(tx) = tx else {
        return Ok(());
    };
    // Run elsewhere, the statement would land outside the transaction the
    // user sees open.
    if tx.database != database {
        return Err(StudioError::TransactionOpen);
    }
    // 0.6.2 drops the transaction on DISCARD ALL, then commits each later
    // statement at once while Studio shows it open.
    if resets_session(sql) {
        return Err(StudioError::TransactionOpen);
    }
    // The server runs ROLLBACK TO in a failed block: it rewinds to the
    // savepoint and clears the failure.
    if tx.failed && !matches!(control, Some(Control::RollbackTo(_))) {
        return Err(StudioError::Rejected {
            reason: "the transaction failed: roll it back, or back to a savepoint, before running more statements"
                .to_string(),
        });
    }
    Ok(())
}

impl NodeDbConnectionService {
    /// Whether the session holds an open transaction. No server call: a
    /// transaction the server already closed reads open until a statement
    /// or `tx_checked` finds the loss.
    pub(super) fn tx_open(&self) -> bool {
        self.session
            .borrow()
            .as_ref()
            .is_some_and(|s| s.tx.is_some())
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
        let (mut lease, id) = {
            let mut session = self.session.borrow_mut();
            let s = session.as_mut().ok_or(StudioError::NotConnected)?;
            let Some(tx) = s.tx.as_ref() else {
                return Ok(TxState::Idle);
            };
            let (state, id, database) = (tx.state(), tx.id, tx.database.clone());
            if !s.conns.get(&database).is_some_and(|c| c.idle()) {
                return Ok(state);
            }
            // A statement runs on it, so it is in use.
            let Some(lease) = self.checkout(s, &database) else {
                return Ok(state);
            };
            (lease, id)
        };
        match failed_on(lease.conn()?).await {
            Ok(failed) => lease.put_back(|tx| {
                if let Some(tx) = tx.as_mut().filter(|tx| tx.id == id) {
                    tx.failed = failed;
                }
                Ok(())
            }),
            Err(Gone(detail)) => {
                lease.end();
                Err(lost(detail))
            }
        }
    }

    /// Run one statement on `database` and return the transaction state
    /// after it. Transaction control moves the session's transaction.
    pub(super) async fn execute(
        &self,
        database: &str,
        sql: &str,
        values: &[Value],
    ) -> Result<Ran, StudioError> {
        // 0.6.2 reads a closing `;` into the value of SET, SHOW and RESET:
        // `SET x = 'a';` stores `a';`.
        let sql = sql.trim_end().trim_end_matches(';').trim_end();
        let control = control_of(sql);
        match control {
            Some(Control::Begin) => return self.begin_tx(database, sql).await,
            Some(Control::Commit) => return self.end_tx(true).await,
            Some(Control::Rollback) => return self.end_tx(false).await,
            Some(Control::Savepoint(_) | Control::Release(_) | Control::RollbackTo(_)) | None => {}
        }
        let mut lease = self
            .take(database, |database, tx| {
                may_run(sql, control.as_ref(), database, tx)
            })
            .await?;
        let reset = lease.reset;
        let answer = match bounded(
            STATEMENT_LIMIT,
            lease.conn()?.execute_sql_with_params(sql, values),
        )
        .await
        {
            Err(Gone(detail)) => {
                let in_tx = lease.tx.is_some();
                lease.end();
                return Err(if in_tx {
                    lost(detail)
                } else {
                    no_answer(detail)
                });
            }
            Ok(answer) => answer,
        };
        let Some(id) = lease.tx else {
            let tx = lease.put_back(|_| Ok(()))?;
            return Ok(Ran::new(answer?, tx, reset));
        };
        // A COMMIT the classifier missed still ends the transaction.
        if let Ok(result) = &answer
            && status_of(result) == Some("COMMIT")
        {
            let tx = lease.put_back(|tx| {
                *tx = None;
                Ok(())
            })?;
            return Ok(Ran::new(answer?, tx, reset));
        }
        // 0.6.2 fails the transaction on a planning or execution error, keeps
        // it open on a DDL error, and clears a failure on ROLLBACK TO. Ask
        // the server which one holds.
        let ask = answer.is_err() || matches!(control, Some(Control::RollbackTo(_)));
        let failed = if ask {
            match failed_on(lease.conn()?).await {
                Ok(failed) => failed,
                Err(Gone(detail)) => {
                    lease.end();
                    return Err(lost(detail));
                }
            }
        } else {
            false
        };
        let succeeded = answer.is_ok();
        let tx = lease.put_back(|tx| match tx.as_mut().filter(|tx| tx.id == id) {
            Some(tx) => {
                if succeeded {
                    tx.ran(control.as_ref());
                }
                tx.failed = failed;
                Ok(())
            }
            None => Err(lost("the session changed while the statement ran")),
        })?;
        Ok(Ran::new(answer?, tx, reset))
    }

    /// Open a transaction on `database` by running `sql`, a BEGIN, on its
    /// statement connection.
    pub(super) async fn begin_tx(&self, database: &str, sql: &str) -> Result<Ran, StudioError> {
        if asks_read_only(sql) {
            return Err(read_only_unsupported());
        }
        let mut lease = self
            .take(database, |_, tx| match tx {
                Some(_) => Err(StudioError::TransactionOpen),
                None => Ok(()),
            })
            .await?;
        let reset = lease.reset;
        let result = match bounded(CONTROL_LIMIT, lease.conn()?.execute_sql(sql)).await {
            Ok(Ok(result)) => result,
            // The server refused BEGIN before it opened a block.
            Ok(Err(e)) => {
                lease.put_back(|_| Ok(()))?;
                return Err(e.into());
            }
            Err(Gone(detail)) => {
                lease.end();
                return Err(StudioError::Connection(
                    NodeDbError::sync_connection_failed(detail),
                ));
            }
        };
        // Anything but the BEGIN tag means Studio cannot tell whether the
        // server opened a block: closing the connection ends any it did.
        if status_of(&result) != Some("BEGIN") {
            lease.end();
            return Err(StudioError::Setup(NodeDbError::bad_request(format!(
                "the server did not open a transaction for `{sql}`"
            ))));
        }
        let database = lease.database().to_string();
        let tx = lease.put_back(|tx| match tx {
            // A BEGIN on another database finished first.
            Some(_) => Err(StudioError::TransactionOpen),
            None => {
                *tx = Some(Tx {
                    id: next_id(),
                    database,
                    statements: 0,
                    savepoints: Vec::new(),
                    failed: false,
                });
                Ok(())
            }
        })?;
        Ok(Ran::new(result, tx, reset))
    }

    /// Commit the open transaction, or roll it back. A failed transaction
    /// always rolls back, with a warning: 0.6.2 answers COMMIT in a failed
    /// block by committing the writes made before the failure.
    pub(super) async fn end_tx(&self, commit: bool) -> Result<Ran, StudioError> {
        let (database, id, failed) = {
            let session = self.session.borrow();
            let s = session.as_ref().ok_or(StudioError::NotConnected)?;
            let tx = s.tx.as_ref().ok_or(StudioError::NoTransaction)?;
            (tx.database.clone(), tx.id, tx.failed)
        };
        // `take` asks the server first when the connection sat idle. A
        // server that closed it meanwhile rolled back: nothing to commit.
        let mut lease = self
            .take(&database, |_, tx| match tx {
                Some(tx) if tx.id == id => Ok(()),
                Some(_) | None => Err(StudioError::NoTransaction),
            })
            .await?;
        let committing = commit && !failed;
        let sql = if committing { "COMMIT" } else { "ROLLBACK" };
        let answer = bounded(CONTROL_LIMIT, lease.conn()?.execute_sql(sql)).await;
        let tx = if matches!(answer, Ok(Ok(_))) {
            lease.put_back(|tx| {
                *tx = None;
                Ok(())
            })?
        } else {
            // A refused or unanswered end: closing the connection rolls
            // back whatever the server still holds.
            lease.end();
            TxState::Idle
        };
        Ok(Ran {
            result: ended(committing, answer)?,
            tx,
            warnings: (commit && failed)
                .then_some(ROLLED_BACK)
                .into_iter()
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use nodedb_client::native::pool::PoolConfig;
    use nodedb_types::protocol::AuthMethod as WireAuth;

    use super::*;
    use crate::services::connection_service::ConnectionService;
    use crate::services::nodedb::service::Session;
    use crate::services::nodedb::workbench_conn::{IDLE_PROBE_AFTER, StmtConn};
    use crate::services::workbench_data::WorkbenchData;
    use crate::state::connections_registry::Credentials;

    fn tx() -> Tx {
        Tx {
            id: next_id(),
            database: "default".into(),
            statements: 0,
            savepoints: Vec::new(),
            failed: false,
        }
    }

    #[test]
    fn rollback_to_a_savepoint_undoes_the_count_after_it() {
        let mut tx = tx();
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
        let mut tx = tx();
        tx.ran(Some(&Control::Savepoint("a".into())));
        tx.ran(None);
        tx.ran(Some(&Control::Savepoint("a".into())));
        tx.ran(None);
        tx.ran(Some(&Control::RollbackTo("a".into())));
        assert_eq!(tx.statements, 4);
        assert_eq!(tx.savepoints.len(), 2);
    }

    /// A service that never reached a server, whose `default` database has
    /// a statement connection last used at `used` with a statement running
    /// on it, and `tx` open there.
    fn holding(tx: Option<Tx>, used: Instant) -> NodeDbConnectionService {
        let config = PoolConfig::new(
            "127.0.0.1:1",
            WireAuth::Trust {
                username: "alice".into(),
            },
        );
        let svc = NodeDbConnectionService::default();
        let mut session = Session::new(config, "default".into(), "0.6.2".into(), "alice");
        session.conns.insert(
            "default".into(),
            StmtConn {
                id: next_id(),
                conn: None,
                used,
            },
        );
        session.tx = tx;
        *svc.session.borrow_mut() = Some(session);
        svc
    }

    fn long_ago() -> Instant {
        Instant::now()
            .checked_sub(IDLE_PROBE_AFTER * 2)
            .expect("the clock runs long enough")
    }

    #[tokio::test]
    async fn the_state_read_asks_the_server_only_after_idle() {
        let active = TxState::Active {
            access: TxAccess::ReadWrite,
            statements: 0,
        };
        // Used just now: no probe, so no network.
        let svc = holding(Some(tx()), Instant::now());
        assert_eq!(svc.tx_checked().await.expect("reads"), active);
        // Idle, but a statement holds the connection: it is in use.
        let svc = holding(Some(tx()), long_ago());
        assert_eq!(svc.tx_checked().await.expect("reads"), active);
    }

    /// One statement at a time on a database's connection, in a
    /// transaction or out of one. Nothing reaches the network.
    #[tokio::test]
    async fn a_second_statement_while_one_runs_is_refused() {
        for tx in [None, Some(tx())] {
            let svc = holding(tx, Instant::now());
            let refused = svc.execute("", "SELECT 1", &[]).await;
            assert!(
                matches!(&refused, Err(StudioError::Setup(e)) if e.to_string().contains("still running")),
                "{:?}",
                refused.map(|r| r.result)
            );
        }
    }

    /// A connect would drop the transaction's connection, and the server
    /// would roll it back unseen. It is refused before anything is looked
    /// up, and the session stays as it was.
    #[tokio::test]
    async fn connect_is_refused_while_a_transaction_is_open() {
        let svc = holding(Some(tx()), Instant::now());
        let creds = Credentials {
            username: "alice".into(),
            password: None,
        };
        assert!(matches!(
            svc.connect("anything", &creds).await,
            Err(StudioError::TransactionOpen)
        ));
        assert!(svc.tx_open());
        assert!(matches!(
            svc.transaction().await,
            Ok(TxState::Active { .. })
        ));
    }

    /// A closed connection takes its transaction with it, and the next
    /// statement on its database says the settings are gone.
    #[test]
    fn closing_a_connection_ends_its_transaction_and_marks_the_reset() {
        let svc = holding(Some(tx()), Instant::now());
        let mut session = svc.session.borrow_mut();
        let s = session.as_mut().expect("a session");
        let id = s.conns["default"].id;
        s.close("other", id);
        s.close("default", id + 1);
        assert!(s.tx.is_some() && s.reset.is_empty(), "not that connection");
        s.close("default", id);
        assert!(s.tx.is_none() && s.conns.is_empty());
        assert!(s.reset.contains("default"));
    }

    mod live {
        use crate::models::console::{ConsoleOutput, HistoryQuery, HistoryWindow, Protocol};
        use crate::models::saved_query::SaveQuery;
        use crate::models::workbench::{MessageLevel, QueryRequest, TxAccess, TxState};
        use crate::services::connection_service::ConnectionService;
        use crate::services::error::StudioError;
        use crate::services::nodedb::NodeDbConnectionService;
        use crate::services::nodedb::service::{live, to_table};
        use crate::services::nodedb::workbench_conn::{SETTINGS_RESET, kill_conn};
        use crate::services::workbench_data::WorkbenchData;
        use crate::state::connections_registry::Credentials;

        /// Kill the open transaction's connection. `idle` makes it look
        /// unused past the probe threshold.
        async fn kill_pinned(svc: &NodeDbConnectionService, idle: bool) {
            let database = svc
                .session
                .borrow()
                .as_ref()
                .and_then(|s| s.tx.as_ref())
                .map(|tx| tx.database.clone())
                .expect("a transaction");
            kill_conn(svc, &database, idle).await;
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
        async fn live_tx_connect_waits_for_the_transaction_to_end() {
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
            // A connect would drop the transaction unseen: the user must end
            // it first, and nothing changes meanwhile.
            assert!(matches!(
                svc.connect("probe", &creds).await,
                Err(StudioError::TransactionOpen)
            ));
            // The INSERT and the console's SELECT.
            assert_eq!(svc.transaction().await.expect("state"), active(2));
            let seen = svc
                .run_query(&q(&format!("SELECT id FROM {t}")))
                .await
                .expect("selects");
            assert_eq!(seen.result.total_rows, 1, "the write is still there");
            svc.rollback_transaction().await.expect("rolls back");
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
            // Outside a transaction it resets the statement connection.
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
            assert!(
                reply
                    .notes
                    .iter()
                    .any(|l| l.contains("Nothing was committed")),
                "{reply:?}"
            );
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
            kill_pinned(&svc, true).await;
            // The badge read finds the loss and reports it once.
            let lost = svc.transaction().await.expect_err("the loss is reported");
            assert!(matches!(lost, StudioError::Connection(_)), "{lost:?}");
            assert_eq!(svc.transaction().await.expect("state"), TxState::Idle);
            assert!(outside(&svc, t).await.is_empty());
            // The next statement runs on a new connection, and says so.
            let next = svc.run_query(&q("SELECT 1")).await.expect("runs");
            assert!(
                next.messages
                    .iter()
                    .any(|m| m.level == MessageLevel::Warning && m.text == SETTINGS_RESET),
                "{:?}",
                next.messages
            );
            let again = svc.run_query(&q("SELECT 1")).await.expect("runs");
            assert!(again.messages.iter().all(|m| m.text != SETTINGS_RESET));
            // COMMIT on a connection the server closed while it sat idle:
            // Studio asks first, and nothing was committed.
            svc.begin_transaction(TxAccess::ReadWrite)
                .await
                .expect("begins");
            svc.run_query(&insert("b")).await.expect("inserts");
            kill_pinned(&svc, true).await;
            let gone = svc.commit_transaction().await.expect_err("lost");
            assert!(
                gone.to_string().contains("uncommitted statements are gone"),
                "{gone}"
            );
            assert!(!gone.to_string().contains("can have committed"), "{gone}");
            assert_eq!(svc.transaction().await.expect("state"), TxState::Idle);
            // COMMIT with no answer: the outcome is unknown, and no Retry.
            svc.begin_transaction(TxAccess::ReadWrite)
                .await
                .expect("begins");
            svc.run_query(&insert("c")).await.expect("inserts");
            kill_pinned(&svc, false).await;
            let unknown = svc.commit_transaction().await.expect_err("no answer");
            assert!(unknown_outcome(&unknown), "{unknown:?}");
            assert_eq!(svc.transaction().await.expect("state"), TxState::Idle);
            svc.console_run(Protocol::Sql, "", "BEGIN")
                .await
                .expect("begins");
            kill_pinned(&svc, false).await;
            let unknown = svc
                .console_run(Protocol::Sql, "", "COMMIT;")
                .await
                .expect_err("no answer");
            assert!(unknown_outcome(&unknown), "{unknown:?}");
            // ROLLBACK with no answer: the server rolled back on close.
            svc.begin_transaction(TxAccess::ReadWrite)
                .await
                .expect("begins");
            kill_pinned(&svc, false).await;
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
