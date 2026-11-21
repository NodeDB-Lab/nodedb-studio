//! Calls on a transaction's pinned connection: opening it, awaiting a call
//! within a time limit, and what a call without an answer means.

use std::future::Future;
use std::time::Duration;

use nodedb_client::native::connection::NativeConnection;
use nodedb_client::native::pool::PoolConfig;
use nodedb_client::{NodeDbError, QueryResult};
use nodedb_types::Value;
use nodedb_types::error::{ErrorCode, ErrorDetails};

use crate::services::error::StudioError;

/// A failed block refuses every statement except transaction control. This
/// read answers in an open block and is refused in a failed one.
const PROBE: &str = "SHOW transaction_read_only";

/// How long one statement on the transaction's connection can run.
pub(super) const STATEMENT_LIMIT: Duration = Duration::from_secs(300);

/// How long BEGIN, COMMIT, ROLLBACK and the probe can take.
pub(super) const CONTROL_LIMIT: Duration = Duration::from_secs(60);

/// The tag of a status row, such as `BEGIN` or `COMMIT`.
pub(super) fn status_of(result: &QueryResult) -> Option<&str> {
    match (result.columns.as_slice(), result.rows.as_slice()) {
        ([column], [row]) if column == "status" => row.first().and_then(Value::as_str),
        _ => None,
    }
}

/// The connection is gone: the client could not write or read the frame.
pub(super) fn is_dead(e: &NodeDbError) -> bool {
    matches!(e.details(), ErrorDetails::SyncConnectionFailed)
}

/// No answer came: the connection died, or the call ran out of time. What
/// the server did with the call is unknown.
#[derive(Debug)]
pub(super) struct Gone(pub String);

/// Await `call` for at most `limit`. The server's answer, success or
/// error, is `Ok`.
pub(super) async fn bounded(
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
pub(super) fn lost(detail: impl std::fmt::Display) -> StudioError {
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

/// The answer to the COMMIT or ROLLBACK that ended a transaction.
pub(super) fn ended(
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
pub(super) async fn failed_on(conn: &mut NativeConnection) -> Result<bool, Gone> {
    Ok(bounded(CONTROL_LIMIT, conn.execute_sql(PROBE))
        .await?
        .is_err())
}

/// Open the connection a transaction pins, as the pool would.
pub(super) async fn open(config: &PoolConfig) -> Result<NativeConnection, StudioError> {
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
