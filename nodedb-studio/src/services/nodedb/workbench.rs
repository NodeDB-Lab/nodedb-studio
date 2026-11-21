//! Query workbench. `run_query` talks to the server. Every other method waits
//! on the real client.

use std::time::Instant;

use async_trait::async_trait;
use nodedb_client::{NodeDb, QueryResult};
use nodedb_types::Value;

use crate::models::console::Protocol;
use crate::models::console::{ConsoleReply, HistoryEntry, HistoryQuery};
use crate::models::explorer::RecordRow;
use crate::models::query_plan::{ExplainMode, QueryPlan};
use crate::models::saved_query::{QueryLibrary, SaveQuery, SavedQuery, SavedQueryVersion};
use crate::models::sql_text::{TokenKind, tokenize};
use crate::models::workbench::{
    MessageLevel, ParamBinding, ParamError, ParamType, QueryDraft, QueryMessage, QueryRequest,
    QueryRun, ResultSet, SchemaTree, TxAccess, TxState,
};
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;
use crate::services::nodedb::service::LastRun;
use crate::services::workbench_data::WorkbenchData;

impl NodeDbConnectionService {
    /// Page `request.page` of the last run, when it ran the same statement
    /// on the same database.
    fn buffered_page(
        &self,
        request: &QueryRequest,
        sql: &str,
    ) -> Result<Option<QueryRun>, StudioError> {
        let session = self.session.borrow();
        let Some(last) = session.as_ref().and_then(|s| s.last_run.as_ref()) else {
            return Ok(None);
        };
        if last.database != request.database || last.sql != sql {
            return Ok(None);
        }
        build_run(
            &last.result,
            request.page,
            request.page_size,
            last.elapsed_ms,
        )
        .map(Some)
    }
}

/// A cell as the grid shows it. A missing value is an en dash, which the
/// grid's numeric alignment skips. A nested object or array is compact JSON,
/// because `Display` for those is not valid JSON.
fn render_cell(value: &Value) -> String {
    match value {
        Value::Null => "\u{2013}".to_string(),
        Value::Object(_) | Value::Array(_) => {
            sonic_rs::to_string(value).unwrap_or_else(|_| value.to_string())
        }
        other => other.to_string(),
    }
}

/// Page `page` of a buffered result. A row id is its index in the whole
/// result, so ids stay unique across pages.
fn build_run(
    result: &QueryResult,
    page: usize,
    page_size: usize,
    elapsed_ms: u32,
) -> Result<QueryRun, StudioError> {
    let page_size = page_size.max(1);
    let total = result.rows.len();
    let start = page.saturating_mul(page_size).min(total);
    let end = start.saturating_add(page_size).min(total);
    let rows = result.rows[start..end]
        .iter()
        .enumerate()
        .map(|(i, row)| RecordRow {
            id: format!("row-{}", start + i),
            cells: row.iter().map(render_cell).collect(),
        })
        .collect();
    let set = ResultSet::new(result.columns.clone(), rows, elapsed_ms, String::new())?
        .with_page(page, page_size, total);
    let messages = if result.columns.is_empty() {
        vec![QueryMessage {
            level: MessageLevel::Info,
            text: outcome_text(result),
        }]
    } else {
        Vec::new()
    };
    Ok(QueryRun {
        result: set,
        messages,
        tx: TxState::Idle,
    })
}

/// What a statement without columns did.
fn outcome_text(result: &QueryResult) -> String {
    let command = result.command.as_deref().unwrap_or("OK");
    match (&result.command, result.rows_affected) {
        (Some(_), n) => format!("{command}: {n} rows affected"),
        (None, 0) => "Statement ran".to_string(),
        (None, n) => format!("{command}: {n} rows affected"),
    }
}

/// Rewrite `:name` bindings to the positional `$n` the server binds, and
/// return the values in that order. Values never enter the SQL text.
fn bind_params(sql: &str, params: &[ParamBinding]) -> Result<(String, Vec<Value>), StudioError> {
    let mut names: Vec<&str> = Vec::new();
    let mut out = String::with_capacity(sql.len());
    for token in tokenize(sql, Protocol::Sql) {
        if token.kind != TokenKind::Param {
            out.push_str(token.text);
            continue;
        }
        let name = &token.text[1..];
        let at = match names.iter().position(|n| *n == name) {
            Some(at) => at,
            None => {
                names.push(name);
                names.len() - 1
            }
        };
        out.push_str(&format!("${}", at + 1));
    }
    let mut values = Vec::with_capacity(names.len());
    for name in names {
        let binding = params
            .iter()
            .find(|p| p.name == name)
            .ok_or_else(|| StudioError::MissingParameter(name.to_string()))?;
        values.push(param_value(binding)?);
    }
    Ok((out, values))
}

fn param_value(binding: &ParamBinding) -> Result<Value, StudioError> {
    binding.check().map_err(|e| match e {
        ParamError::Missing { name } => StudioError::MissingParameter(name),
        ParamError::Mismatch { .. } => StudioError::Rejected {
            reason: e.to_string(),
        },
    })?;
    let v = binding.value.trim();
    // `check` accepted the text. A parse that fails anyway is refused: a
    // silent NULL would run the statement with the wrong value.
    let refused = || StudioError::Rejected {
        reason: format!(
            "parameter :{} does not parse as {}",
            binding.name,
            binding.ty.label()
        ),
    };
    Ok(match binding.ty {
        ParamType::Text => Value::String(binding.value.clone()),
        ParamType::Integer => Value::Integer(v.parse().map_err(|_| refused())?),
        ParamType::Decimal => Value::Float(v.parse().map_err(|_| refused())?),
        ParamType::Boolean => Value::Bool(v == "true"),
        ParamType::Timestamp => Value::String(v.to_string()),
    })
}

#[async_trait(?Send)]
impl WorkbenchData for NodeDbConnectionService {
    async fn run_query(&self, request: &QueryRequest) -> Result<QueryRun, StudioError> {
        let sql = request.sql.trim();
        if sql.is_empty() {
            return Err(StudioError::EmptyStatement);
        }
        // A page change reads the buffered rows. Executing again would repeat
        // a write.
        if request.page > 0
            && let Some(run) = self.buffered_page(request, sql)?
        {
            return Ok(run);
        }
        let client = self.client(Some(&request.database))?;
        let started = Instant::now();
        let result = if request.params.is_empty() {
            client.query(sql).await?
        } else {
            let (positional, values) = bind_params(sql, &request.params)?;
            client.execute_sql(&positional, &values).await?
        };
        let elapsed_ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
        let run = build_run(&result, request.page, request.page_size, elapsed_ms)?;
        if let Some(session) = self.session.borrow_mut().as_mut() {
            session.last_run = Some(LastRun {
                database: request.database.clone(),
                sql: sql.to_string(),
                result,
                elapsed_ms,
            });
        }
        Ok(run)
    }

    async fn explain(
        &self,
        _request: &QueryRequest,
        _mode: ExplainMode,
    ) -> Result<QueryPlan, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn schema_tree(&self) -> Result<SchemaTree, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn query_session(&self) -> Result<Vec<QueryDraft>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn transaction(&self) -> Result<TxState, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn begin_transaction(&self, _access: TxAccess) -> Result<TxState, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn commit_transaction(&self) -> Result<TxState, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn rollback_transaction(&self) -> Result<TxState, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn query_library(&self) -> Result<QueryLibrary, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn saved_query_versions(&self, _id: &str) -> Result<Vec<SavedQueryVersion>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn save_query(&self, _save: &SaveQuery) -> Result<SavedQuery, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn delete_saved_query(&self, _id: &str) -> Result<(), StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn create_query_folder(&self, _name: &str) -> Result<(), StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn console_run(
        &self,
        _protocol: Protocol,
        _database: &str,
        _text: &str,
    ) -> Result<ConsoleReply, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn console_history(
        &self,
        _query: &HistoryQuery,
    ) -> Result<Vec<HistoryEntry>, StudioError> {
        Err(StudioError::NotConnected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::nodedb::service::live;

    fn q(sql: &str, page: usize, page_size: usize) -> QueryRequest {
        QueryRequest {
            sql: sql.into(),
            params: Vec::new(),
            database: String::new(),
            page,
            page_size,
        }
    }

    fn result(n: usize) -> QueryResult {
        QueryResult {
            columns: vec!["n".into()],
            rows: (0..n as i64).map(|i| vec![Value::Integer(i)]).collect(),
            rows_affected: 0,
            command: None,
        }
    }

    #[test]
    fn pages_slice_with_absolute_ids() {
        let run = build_run(&result(5), 1, 2, 7).expect("builds");
        let ids: Vec<_> = run.result.rows().iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["row-2", "row-3"]);
        assert_eq!(run.result.total_rows, 5);
        assert_eq!(run.result.pages(), 3);
        let last = build_run(&result(5), 2, 2, 7).expect("builds");
        assert_eq!(last.result.rows().len(), 1);
    }

    #[test]
    fn a_page_past_the_end_is_empty_and_a_zero_page_size_is_one() {
        let past = build_run(&result(3), 9, 2, 0).expect("builds");
        assert!(past.result.rows().is_empty());
        let zero = build_run(&result(3), 0, 0, 0).expect("builds");
        assert_eq!(zero.result.rows().len(), 1);
    }

    #[test]
    fn cells_render_null_and_nested_values() {
        assert_eq!(render_cell(&Value::Null), "\u{2013}");
        assert_eq!(render_cell(&Value::Integer(4)), "4");
        let nested = Value::Array(vec![Value::Integer(1), Value::String("a".into())]);
        assert_eq!(render_cell(&nested), r#"[1,"a"]"#);
        let object = Value::Object([("k".to_string(), Value::Bool(true))].into());
        assert_eq!(render_cell(&object), r#"{"k":true}"#);
    }

    #[test]
    fn a_columnless_result_reports_its_command() {
        let done = QueryResult {
            columns: Vec::new(),
            rows: Vec::new(),
            rows_affected: 3,
            command: Some("UPDATE".into()),
        };
        let run = build_run(&done, 0, 50, 1).expect("builds");
        assert!(run.result.rows().is_empty());
        assert_eq!(run.messages.len(), 1);
        assert_eq!(run.messages[0].text, "UPDATE: 3 rows affected");
    }

    #[test]
    fn named_parameters_become_positional() {
        let params = [
            ParamBinding::new("b", ParamType::Integer, "7"),
            ParamBinding::new("a", ParamType::Text, "x"),
        ];
        let (sql, values) =
            bind_params("SELECT ':a' , :a, :b, x::int FROM t WHERE y = :a", &params)
                .expect("binds");
        assert_eq!(sql, "SELECT ':a' , $1, $2, x::int FROM t WHERE y = $1");
        assert_eq!(values, [Value::String("x".into()), Value::Integer(7)]);
    }

    #[test]
    fn an_unbound_or_mistyped_parameter_is_an_error() {
        assert!(matches!(
            bind_params("SELECT :a", &[]),
            Err(StudioError::MissingParameter(n)) if n == "a"
        ));
        let bad = [ParamBinding::new("a", ParamType::Integer, "x")];
        assert!(matches!(
            bind_params("SELECT :a", &bad),
            Err(StudioError::Rejected { .. })
        ));
    }

    #[tokio::test]
    async fn blank_statement_is_empty_without_a_connection() {
        let svc = NodeDbConnectionService::default();
        assert!(matches!(
            svc.run_query(&q("  \n", 0, 50)).await,
            Err(StudioError::EmptyStatement)
        ));
    }

    async fn seed(svc: &NodeDbConnectionService, table: &str) {
        let c = svc.client(None).expect("a client");
        for sql in [
            format!("DROP COLLECTION IF EXISTS {table}"),
            format!("CREATE COLLECTION {table}"),
        ] {
            c.query(&sql).await.expect("seed runs");
        }
    }

    #[tokio::test]
    #[ignore = "needs a live NodeDB 0.6.2 server: see `live`"]
    async fn live_query_selects_columns_and_rows() {
        let svc = live().await;
        seed(&svc, "studio_probe_q_sel").await;
        let insert = "INSERT INTO studio_probe_q_sel (id, n, o) VALUES ('a', 1, {k: [1,2]})";
        svc.run_query(&q(insert, 0, 50)).await.expect("inserts");
        let run = svc
            .run_query(&q("SELECT id, n, o FROM studio_probe_q_sel", 0, 50))
            .await
            .expect("selects");
        assert_eq!(run.result.columns(), ["id", "n", "o"]);
        assert_eq!(run.result.rows().len(), 1);
        assert_eq!(run.result.rows()[0].cells, ["a", "1", r#"{"k":[1,2]}"#]);
        assert!(run.messages.is_empty());
        assert_eq!(run.tx, TxState::Idle);
    }

    #[tokio::test]
    #[ignore = "needs a live NodeDB 0.6.2 server: see `live`"]
    async fn live_query_pages_without_running_again() {
        let svc = live().await;
        seed(&svc, "studio_probe_q_page").await;
        // A second run of this statement would hit the duplicate ids.
        let insert = "INSERT INTO studio_probe_q_page (id, n) VALUES ('x1', 1), ('x2', 2), ('x3', 3) RETURNING id";
        let first = svc.run_query(&q(insert, 0, 2)).await.expect("inserts");
        assert_eq!(first.result.total_rows, 3);
        assert_eq!(first.result.rows().len(), 2);
        let second = svc.run_query(&q(insert, 1, 2)).await.expect("pages");
        let ids: Vec<_> = second.result.rows().iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["row-2"]);
        assert_eq!(second.result.rows()[0].cells, ["x3"]);
        let count = svc
            .run_query(&q("SELECT id FROM studio_probe_q_page", 0, 50))
            .await
            .expect("counts");
        assert_eq!(count.result.total_rows, 3);
        // A page of a statement that was not just run executes.
        let other = svc
            .run_query(&q("SELECT id FROM studio_probe_q_page", 1, 2))
            .await
            .expect("pages");
        assert_eq!(other.result.rows().len(), 1);
    }

    #[tokio::test]
    #[ignore = "needs a live NodeDB 0.6.2 server: see `live`"]
    async fn live_query_write_reports_affected_rows() {
        let svc = live().await;
        seed(&svc, "studio_probe_q_wr").await;
        let insert = "INSERT INTO studio_probe_q_wr (id, n) VALUES ('a', 1)";
        let run = svc.run_query(&q(insert, 0, 50)).await.expect("inserts");
        assert!(run.result.columns().is_empty());
        assert_eq!(run.messages.len(), 1);
        assert_eq!(run.messages[0].level, MessageLevel::Info);
        assert_eq!(run.messages[0].text, "INSERT: 1 rows affected");
        let update = "UPDATE studio_probe_q_wr SET n = 2 WHERE id = 'a'";
        let run = svc.run_query(&q(update, 0, 50)).await.expect("updates");
        assert_eq!(run.messages[0].text, "UPDATE: 1 rows affected");
    }

    #[tokio::test]
    #[ignore = "needs a live NodeDB 0.6.2 server: see `live`"]
    async fn live_query_binds_parameters_and_reports_errors() {
        let svc = live().await;
        let mut request = q("SELECT :n, :s", 0, 50);
        request.params = vec![
            ParamBinding::new("n", ParamType::Integer, "3"),
            ParamBinding::new("s", ParamType::Text, "it's"),
        ];
        let run = svc.run_query(&request).await.expect("binds");
        assert_eq!(run.result.rows()[0].cells, ["3", "it's"]);
        let err = svc
            .run_query(&q("SELEC 1", 0, 50))
            .await
            .expect_err("syntax");
        // The server's plan error maps to `Setup`, never to `Syntax`.
        assert!(matches!(err, StudioError::Setup(_)), "got {err:?}");
        assert!(err.to_string().contains("parse error"), "got {err}");
    }
}
