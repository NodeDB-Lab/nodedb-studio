use std::time::Instant;

use async_trait::async_trait;
use nodedb_client::{NodeDb, NodeDbError, QueryResult};
use nodedb_types::Value;

use crate::models::collection::Profile;
use crate::models::explorer::RecordRow;
use crate::models::schema::CollectionSchema;
use crate::models::workbench::console::{
    ConsoleOutput, ConsoleReply, HistoryEntry, HistoryQuery, Protocol,
};
use crate::models::workbench::query_plan::{ExplainMode, QueryPlan};
use crate::models::workbench::saved_query::{
    QueryLibrary, SaveQuery, SavedQuery, SavedQueryVersion,
};
use crate::models::workbench::sql_text::{TokenKind, tokenize};
use crate::models::workbench::{
    MessageLevel, ParamBinding, ParamError, ParamType, QueryDraft, QueryMessage, QueryRequest,
    QueryRun, ResultSet, SchemaField, SchemaKind, SchemaTable, SchemaTree, TxAccess, TxState,
};
use crate::services::decode::decode_rows;
use crate::services::error::StudioError;
use crate::services::explorer_data::ExplorerData;
use crate::services::nodedb::NodeDbConnectionService;
use crate::services::nodedb::service::{LastRun, to_table};
use crate::services::nodedb::workbench::sql::{is_plain_read, masked};
use crate::services::nodedb::workbench::tx::read_only_unsupported;
use crate::services::workbench_data::WorkbenchData;

impl NodeDbConnectionService {
    /// Page `request.page` of the last run, when it ran the same statement
    /// with the same bound `values` on the same database.
    fn buffered_page(
        &self,
        request: &QueryRequest,
        sql: &str,
        values: &[Value],
    ) -> Result<Option<QueryRun>, StudioError> {
        let tx = self.tx_state()?;
        let session = self.session.borrow();
        let Some(last) = session.as_ref().and_then(|s| s.last_run.as_ref()) else {
            return Ok(None);
        };
        if last.database != request.database || last.sql != sql || last.values != values {
            return Ok(None);
        }
        build_run(
            &last.result,
            request.page,
            request.page_size,
            last.elapsed_ms,
            tx,
        )
        .map(Some)
    }
}

fn ms_since(started: Instant) -> u32 {
    u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX)
}

/// A page of a statement whose rows Studio no longer holds, and that can
/// write. Running it again to read the page would repeat the write.
fn run_again_first() -> StudioError {
    StudioError::Setup(NodeDbError::bad_request(
        "Studio no longer holds the rows of this statement, and running it again can repeat a write: run the statement again, then change the page",
    ))
}

/// 0.6.2 has no EXPLAIN ANALYZE: it reports a parse error. Studio does not
/// run the statement itself to time it.
fn analyze_unsupported() -> StudioError {
    StudioError::Setup(NodeDbError::config(
        "NodeDB 0.6.2 has no EXPLAIN ANALYZE, so nothing measures this plan. Studio did not run the statement.",
    ))
}

/// A collection's declared fields, the id field marked as the key.
fn fields_of(schema: &CollectionSchema) -> Vec<SchemaField> {
    schema
        .fields
        .iter()
        .map(|f| SchemaField {
            name: f.name.clone(),
            type_name: f.ty.label(),
            key: schema.id_field.as_deref() == Some(f.name.as_str()),
        })
        .collect()
}

/// A console reply: rows under a header, or the line saying what ran.
fn console_output(result: &QueryResult) -> ConsoleOutput {
    if result.columns.is_empty() {
        return ConsoleOutput::Text(vec![outcome_text(result)]);
    }
    ConsoleOutput::Table {
        columns: result.columns.clone(),
        rows: result
            .rows
            .iter()
            .map(|row| row.iter().map(render_cell).collect())
            .collect(),
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
    tx: TxState,
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
        tx,
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
        let (positional, values) = if request.params.is_empty() {
            (sql.to_string(), Vec::new())
        } else {
            bind_params(sql, &request.params)?
        };
        // A page change reads the buffered rows. Executing again would repeat
        // a write, so only a plain read runs again for a page.
        if request.page > 0 {
            if let Some(run) = self.buffered_page(request, sql, &values)? {
                return Ok(run);
            }
            if !is_plain_read(sql) {
                return Err(run_again_first());
            }
        }
        let started = Instant::now();
        let ran = self
            .execute(&request.database, &positional, &values)
            .await?;
        let elapsed_ms = ms_since(started);
        let mut run = build_run(
            &ran.result,
            request.page,
            request.page_size,
            elapsed_ms,
            ran.tx,
        )?;
        run.messages
            .extend(ran.warnings.iter().map(|text| QueryMessage {
                level: MessageLevel::Warning,
                text: text.to_string(),
            }));
        if let Some(session) = self.session.borrow_mut().as_mut() {
            session.last_run = Some(LastRun {
                database: request.database.clone(),
                sql: sql.to_string(),
                values,
                result: ran.result,
                elapsed_ms,
            });
        }
        Ok(run)
    }

    async fn explain(
        &self,
        request: &QueryRequest,
        mode: ExplainMode,
    ) -> Result<QueryPlan, StudioError> {
        let sql = request.sql.trim();
        if sql.is_empty() {
            return Err(StudioError::EmptyStatement);
        }
        match mode {
            ExplainMode::Explain => {}
            ExplainMode::Analyze => return Err(analyze_unsupported()),
        }
        // The pool plans it: EXPLAIN never runs the statement, so it stays
        // out of the transaction.
        let (sql, values) = bind_params(sql, &request.params)?;
        let client = self.client(Some(&request.database))?;
        // The server matches `EXPLAIN ` in upper case only.
        let result = client
            .execute_sql(&format!("EXPLAIN {sql}"), &values)
            .await?;
        let plan = decode_rows(&to_table(&result), &["plan"], |r| {
            Ok(r.field("plan")?.to_string())
        })?;
        Ok(QueryPlan::from_text(mode, plan.join("\n")))
    }

    async fn schema_tree(&self) -> Result<SchemaTree, StudioError> {
        let listed = self.table("SHOW MATERIALIZED VIEWS").await?;
        let views = decode_rows(&listed, &["name"], |r| Ok(r.field("name")?.to_string()))?;
        let mut tree = SchemaTree::default();
        // ponytail: a DESCRIBE and a SHOW INDEXES per collection, after the
        // catalog's own DESCRIBE each. Batch them if a large catalog makes
        // the panel slow.
        for c in self.catalog().await? {
            // An array has no DESCRIBE. A collection dropped since the
            // catalog read is not in the schema.
            let fields = if c.profile == Profile::Array {
                Vec::new()
            } else {
                match self.collection_schema(&c.name).await {
                    Ok(schema) => fields_of(&schema),
                    Err(StudioError::NotFound(_)) => continue,
                    Err(e) => return Err(e),
                }
            };
            // A materialized view is stored as a collection, and the
            // catalog lists it with them.
            let is_view = views.contains(&c.name);
            let table = SchemaTable {
                kind: if is_view {
                    SchemaKind::MaterializedView
                } else {
                    SchemaKind::Collection(c.profile)
                },
                name: c.name,
                rows: c.rows,
                fields,
            };
            if is_view {
                tree.views.push(table);
            } else {
                tree.tables.push(table);
            }
        }
        // A streaming view has no collection, so no fields to read.
        for name in views {
            if !tree.views.iter().any(|v| v.name == name) {
                tree.views.push(SchemaTable {
                    name,
                    kind: SchemaKind::MaterializedView,
                    rows: None,
                    fields: Vec::new(),
                });
            }
        }
        Ok(tree)
    }

    async fn query_session(&self) -> Result<Vec<QueryDraft>, StudioError> {
        // Nothing records this backend's tabs yet, so a session restores none.
        self.local(|_| Ok(Vec::new()))
    }

    async fn transaction(&self) -> Result<TxState, StudioError> {
        self.tx_checked().await
    }

    async fn begin_transaction(&self, access: TxAccess) -> Result<TxState, StudioError> {
        match access {
            TxAccess::ReadOnly => Err(read_only_unsupported()),
            TxAccess::ReadWrite => self.begin_tx("", "BEGIN").await.map(|ran| ran.tx),
        }
    }

    async fn commit_transaction(&self) -> Result<TxState, StudioError> {
        self.end_tx(true).await.map(|ran| ran.tx)
    }

    async fn rollback_transaction(&self) -> Result<TxState, StudioError> {
        self.end_tx(false).await.map(|ran| ran.tx)
    }

    async fn query_library(&self) -> Result<QueryLibrary, StudioError> {
        self.local(|s| Ok(s.library()))
    }

    async fn saved_query_versions(&self, id: &str) -> Result<Vec<SavedQueryVersion>, StudioError> {
        self.local(|s| s.versions(id))
    }

    async fn save_query(&self, save: &SaveQuery) -> Result<SavedQuery, StudioError> {
        self.local(|s| s.save(save))
    }

    async fn delete_saved_query(&self, id: &str) -> Result<(), StudioError> {
        self.local(|s| s.delete(id))
    }

    async fn create_query_folder(&self, name: &str) -> Result<(), StudioError> {
        self.local(|s| s.create_folder(name))
    }

    async fn console_run(
        &self,
        protocol: Protocol,
        database: &str,
        text: &str,
    ) -> Result<ConsoleReply, StudioError> {
        let text = text.trim();
        if text.trim_end_matches(';').trim().is_empty() {
            return Err(StudioError::EmptyStatement);
        }
        // The history keeps what the user ran, whatever the server answers,
        // with any password masked.
        self.local(|s| {
            s.record(protocol, &masked(text));
            Ok(())
        })?;
        // SQL, admin commands such as `SHOW DATABASES`, and Cypher `MATCH`
        // all reach the server as statement text, so one path runs all three.
        let started = Instant::now();
        let ran = self.execute(database, text, &[]).await?;
        Ok(ConsoleReply {
            output: console_output(&ran.result),
            elapsed_ms: ms_since(started),
            tx: ran.tx,
            notes: ran.warnings.iter().map(|w| w.to_string()).collect(),
        })
    }

    async fn console_history(
        &self,
        query: &HistoryQuery,
    ) -> Result<Vec<HistoryEntry>, StudioError> {
        self.local(|s| Ok(s.history(query)))
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
        let run = build_run(&result(5), 1, 2, 7, TxState::Idle).expect("builds");
        let ids: Vec<_> = run.result.rows().iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["row-2", "row-3"]);
        assert_eq!(run.result.total_rows, 5);
        assert_eq!(run.result.pages(), 3);
        let last = build_run(&result(5), 2, 2, 7, TxState::Idle).expect("builds");
        assert_eq!(last.result.rows().len(), 1);
    }

    #[test]
    fn a_page_past_the_end_is_empty_and_a_zero_page_size_is_one() {
        let past = build_run(&result(3), 9, 2, 0, TxState::Idle).expect("builds");
        assert!(past.result.rows().is_empty());
        let zero = build_run(&result(3), 0, 0, 0, TxState::Idle).expect("builds");
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
        let run = build_run(&done, 0, 50, 1, TxState::Idle).expect("builds");
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
        // A page of a read that was not just run executes.
        let other = svc
            .run_query(&q("SELECT id FROM studio_probe_q_page", 1, 2))
            .await
            .expect("pages");
        assert_eq!(other.result.rows().len(), 1);
        // A page of a write that was not just run does not run it again,
        // in a transaction or out of one.
        let again = svc.run_query(&q(insert, 1, 2)).await;
        assert!(matches!(again, Err(StudioError::Setup(_))), "{again:?}");
        svc.begin_transaction(TxAccess::ReadWrite)
            .await
            .expect("begins");
        let again = svc.run_query(&q(insert, 1, 2)).await;
        assert!(matches!(again, Err(StudioError::Setup(_))), "{again:?}");
        assert!(matches!(
            svc.transaction().await,
            Ok(TxState::Active { .. })
        ));
        svc.rollback_transaction().await.expect("rolls back");
        let count = svc
            .run_query(&q("SELECT id FROM studio_probe_q_page", 0, 50))
            .await
            .expect("counts");
        assert_eq!(count.result.total_rows, 3);
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

    /// A setting the user makes holds for their next statement, though
    /// Studio's own reads keep several pooled connections busy. The pool
    /// hands out its oldest idle connection, so a pooled statement after
    /// two concurrent reads lands on another connection than the SET.
    #[tokio::test]
    #[ignore = "needs a live NodeDB 0.6.2 server: see `live`"]
    async fn live_query_session_settings_stick() {
        use crate::services::connection_service::ConnectionService;
        let svc = live().await;
        let c = svc.client(None).expect("a client");
        // 0.6.2 plans `CREATE SEQUENCE IF NOT EXISTS` for a new sequence and
        // refuses it. The plain form fails when an earlier run made it.
        let _ = c.query("CREATE SEQUENCE studio_probe_w_seq").await;
        let busy = || async {
            let (a, b) = tokio::join!(svc.databases(), svc.databases());
            assert!(a.is_ok() && b.is_ok(), "{a:?} {b:?}");
        };
        busy().await;
        svc.run_query(&q("SET application_name = 'studio_probe_w1'", 0, 50))
            .await
            .expect("sets");
        // The pool held two idle connections: this one gets the other.
        let shown = svc
            .run_query(&q("SHOW application_name", 0, 50))
            .await
            .expect("shows");
        assert_eq!(shown.result.rows()[0].cells, ["studio_probe_w1"]);
        // The Console shares the session, and a closing `;` stays out of
        // the value.
        let show = |want: &str| ConsoleOutput::Table {
            columns: vec!["setting".into()],
            rows: vec![vec![want.into()]],
        };
        let typed = svc
            .console_run(Protocol::Sql, "", "SHOW application_name;")
            .await
            .expect("shows");
        assert_eq!(typed.output, show("studio_probe_w1"));
        svc.console_run(
            Protocol::Sql,
            "",
            "SET application_name = 'studio_probe_w2';",
        )
        .await
        .expect("sets");
        busy().await;
        let shown = svc
            .run_query(&q("SHOW application_name", 0, 50))
            .await
            .expect("shows");
        assert_eq!(shown.result.rows()[0].cells, ["studio_probe_w2"]);
        // Studio's own reads stay on the pool, out of the user's session.
        for _ in 0..3 {
            let pooled = c.query("SHOW application_name").await.expect("reads");
            assert_ne!(to_table(&pooled).rows, [["studio_probe_w1"]]);
        }
        busy().await;
        let next = svc
            .run_query(&q("SELECT nextval('studio_probe_w_seq') AS v", 0, 50))
            .await
            .expect("advances");
        let current = svc
            .run_query(&q("SELECT currval('studio_probe_w_seq') AS v", 0, 50))
            .await
            .expect("reads the session's own value");
        assert_eq!(current.result.rows()[0].cells, next.result.rows()[0].cells);
        // A connection the server closed while it sat idle is replaced, and
        // the user reads that the settings are gone, because they are.
        crate::services::nodedb::workbench::conn::kill_conn(&svc, "default", true).await;
        let fresh = svc
            .console_run(Protocol::Sql, "default", "SHOW application_name")
            .await
            .expect("runs on a new connection");
        assert_eq!(fresh.output, show(""));
        assert_eq!(
            fresh.notes,
            [crate::services::nodedb::workbench::conn::SETTINGS_RESET]
        );
        let again = svc.run_query(&q("SELECT 1", 0, 50)).await.expect("runs");
        assert!(again.messages.is_empty(), "{:?}", again.messages);
    }

    /// Sign `user` of tenant 1 in on saved connection `probe`, as `connect`
    /// does, with a session that never reached a server.
    fn sign_in(svc: &NodeDbConnectionService, user: &str) {
        sign_in_to(svc, user, "1");
    }

    fn sign_in_to(svc: &NodeDbConnectionService, user: &str, tenant: &str) {
        use nodedb_client::native::pool::PoolConfig;
        use nodedb_types::protocol::AuthMethod as WireAuth;
        let config = PoolConfig::new(
            "127.0.0.1:1",
            WireAuth::Trust {
                username: user.into(),
            },
        );
        let mut session = crate::services::nodedb::service::Session::new(
            config,
            "default".into(),
            "0.6.2".into(),
            user,
        );
        session.connection = "probe".into();
        session.tenant_id = tenant.into();
        *svc.session.borrow_mut() = Some(session);
    }

    /// Two tenants can each have an `alice`. Neither reads the other's
    /// saved queries or console history.
    #[tokio::test]
    async fn the_same_user_name_in_another_tenant_has_its_own_records() {
        let svc = offline();
        let mine = svc.save_query(&save("Mine")).await.expect("saves");
        // Recorded before the statement fails to reach a server.
        let _ = svc.console_run(Protocol::Sql, "", "SELECT 'mine'").await;
        sign_in_to(&svc, "alice", "2");
        assert!(svc.query_library().await.expect("reads").queries.is_empty());
        let all = HistoryQuery {
            protocol: None,
            window: crate::models::workbench::console::HistoryWindow::AllTime,
        };
        assert!(svc.console_history(&all).await.expect("reads").is_empty());
        sign_in_to(&svc, "alice", "1");
        assert_eq!(svc.query_library().await.expect("reads").queries, [mine]);
        assert_eq!(svc.console_history(&all).await.expect("reads").len(), 1);
    }

    /// The history keeps a typed password masked, whatever the server
    /// answers.
    #[tokio::test]
    async fn the_console_history_masks_a_password() {
        let svc = offline();
        let typed = "CREATE USER bob WITH PASSWORD 'hunter''2' ROLE readonly;";
        assert!(svc.console_run(Protocol::Sql, "", typed).await.is_err());
        let all = HistoryQuery {
            protocol: None,
            window: crate::models::workbench::console::HistoryWindow::AllTime,
        };
        let history = svc.console_history(&all).await.expect("reads");
        let texts: Vec<&str> = history.iter().map(|h| h.text.as_str()).collect();
        assert_eq!(
            texts,
            ["CREATE USER bob WITH PASSWORD '********' ROLE readonly;"]
        );
    }

    /// A page change reuses the buffered rows only for the same bound
    /// values. Other values run the statement again.
    #[tokio::test]
    async fn a_page_of_the_last_run_with_other_values_runs_again() {
        let svc = offline();
        let sql = "SELECT id FROM t WHERE c = :c";
        if let Some(s) = svc.session.borrow_mut().as_mut() {
            s.last_run = Some(LastRun {
                database: String::new(),
                sql: sql.into(),
                values: vec![Value::String("alice".into())],
                result: result(3),
                elapsed_ms: 1,
            });
        }
        let page = |name: &str| {
            let mut request = q(sql, 1, 2);
            request.params = vec![ParamBinding::new("c", ParamType::Text, name)];
            request
        };
        let same = svc.run_query(&page("alice")).await.expect("buffered");
        assert_eq!(same.result.rows()[0].cells, ["2"]);
        // No server answers here: running again is the only way to fail.
        assert!(
            svc.run_query(&page("bob")).await.is_err(),
            "bob's page is not alice's rows"
        );
    }

    /// A session that never reached a server: everything Studio keeps
    /// itself answers, and nothing touches the network.
    fn offline() -> NodeDbConnectionService {
        let svc = NodeDbConnectionService::default();
        sign_in(&svc, "alice");
        svc
    }

    #[tokio::test]
    async fn saved_queries_outlive_a_reconnect_and_belong_to_their_user() {
        let svc = offline();
        let made = svc.save_query(&save("Kept")).await.expect("saves");
        sign_in(&svc, "alice");
        let lib = svc.query_library().await.expect("reads");
        assert_eq!(
            lib.queries,
            std::slice::from_ref(&made),
            "a reconnect keeps them"
        );
        // Another user of the connection has a library of their own.
        sign_in(&svc, "bob");
        assert!(svc.query_library().await.expect("reads").queries.is_empty());
        let theirs = svc.save_query(&save("Theirs")).await.expect("saves");
        assert_eq!(theirs.owner, "bob");
        sign_in(&svc, "alice");
        let lib = svc.query_library().await.expect("reads");
        assert_eq!(lib.queries, [made]);
    }

    /// A page of a write Studio no longer buffers is refused before
    /// anything reaches the server.
    #[tokio::test]
    async fn a_page_of_an_unbuffered_write_does_not_run_it() {
        let svc = offline();
        for sql in [
            "INSERT INTO t (id) VALUES ('a') RETURNING id",
            "WITH d AS (DELETE FROM t RETURNING id) SELECT * FROM d",
            "COMMIT",
        ] {
            let refused = svc.run_query(&q(sql, 1, 50)).await;
            assert!(
                matches!(&refused, Err(StudioError::Setup(e)) if e.to_string().contains("run the statement again")),
                "{sql}: {refused:?}"
            );
        }
    }

    fn save(name: &str) -> SaveQuery {
        SaveQuery {
            id: None,
            name: name.into(),
            folder: None,
            database: "default".into(),
            sql: "SELECT 1".into(),
            params: Vec::new(),
            tags: Vec::new(),
        }
    }

    #[tokio::test]
    async fn the_library_starts_empty_and_saves_as_the_signed_in_user() {
        let svc = offline();
        assert_eq!(
            svc.query_library().await.expect("reads"),
            QueryLibrary::default()
        );
        assert!(svc.query_session().await.expect("reads").is_empty());
        let made = svc.save_query(&save("Weekly")).await.expect("saves");
        assert_eq!((made.owner.as_str(), made.version), ("alice", 1));
        let mut again = save("Weekly v2");
        again.id = Some(made.id.clone());
        let v2 = svc.save_query(&again).await.expect("versions");
        assert_eq!(v2.version, 2);
        let versions = svc.saved_query_versions(&made.id).await.expect("reads");
        assert_eq!(
            versions.iter().map(|v| v.version).collect::<Vec<_>>(),
            [2, 1]
        );
        assert!(versions.iter().all(|v| v.author == "alice"));
        assert!(matches!(
            svc.save_query(&save("weekly V2")).await,
            Err(StudioError::QueryNameTaken(_))
        ));
        svc.create_query_folder("Finance").await.expect("folder");
        svc.delete_saved_query(&made.id).await.expect("deletes");
        let lib = svc.query_library().await.expect("reads");
        assert!(lib.queries.is_empty());
        assert_eq!(lib.folders, ["Finance"]);
    }

    #[tokio::test]
    async fn saved_queries_carry_the_wall_clock() {
        let svc = offline();
        let before = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("after the epoch")
            .as_secs() as i64;
        let made = svc.save_query(&save("Now")).await.expect("saves");
        assert!((before..before + 5).contains(&made.updated_at));
    }

    #[tokio::test]
    async fn offline_transaction_answers_need_no_server() {
        let svc = offline();
        assert_eq!(svc.transaction().await.expect("reads"), TxState::Idle);
        assert!(matches!(
            svc.commit_transaction().await,
            Err(StudioError::NoTransaction)
        ));
        assert!(matches!(
            svc.rollback_transaction().await,
            Err(StudioError::NoTransaction)
        ));
        assert!(matches!(
            svc.begin_transaction(TxAccess::ReadOnly).await,
            Err(StudioError::Setup(_))
        ));
        assert!(matches!(
            svc.explain(&q("SELECT 1", 0, 50), ExplainMode::Analyze)
                .await,
            Err(StudioError::Setup(_))
        ));
        assert!(matches!(
            svc.console_run(Protocol::Sql, "", " ; ").await,
            Err(StudioError::EmptyStatement)
        ));
    }

    #[tokio::test]
    async fn no_session_keeps_no_records() {
        let svc = NodeDbConnectionService::default();
        assert!(matches!(
            svc.query_session().await,
            Err(StudioError::NotConnected)
        ));
        assert!(matches!(
            svc.console_history(&HistoryQuery {
                protocol: None,
                window: crate::models::workbench::console::HistoryWindow::AllTime,
            })
            .await,
            Err(StudioError::NotConnected)
        ));
    }

    #[test]
    fn console_output_is_a_table_or_the_outcome_line() {
        assert_eq!(
            console_output(&result(1)),
            ConsoleOutput::Table {
                columns: vec!["n".into()],
                rows: vec![vec!["0".into()]],
            }
        );
        let done = QueryResult {
            columns: Vec::new(),
            rows: Vec::new(),
            rows_affected: 2,
            command: Some("DELETE".into()),
        };
        assert_eq!(
            console_output(&done),
            ConsoleOutput::Text(vec!["DELETE: 2 rows affected".into()])
        );
    }

    fn history(protocol: Option<Protocol>) -> HistoryQuery {
        HistoryQuery {
            protocol,
            window: crate::models::workbench::console::HistoryWindow::Day,
        }
    }

    #[tokio::test]
    #[ignore = "needs a live NodeDB 0.6.2 server: see `live`"]
    async fn live_console_refuses_a_statement_recalled_with_a_masked_password() {
        use crate::services::nodedb::service::live_as;
        use crate::services::nodedb::workbench::sql::masked;

        let svc = live().await;
        let user = "studio_probe_w_mask";
        // Fails when the user exists from an earlier run: that is fine.
        let _ = svc
            .console_run(
                Protocol::Sql,
                "",
                &format!("CREATE USER {user} WITH PASSWORD 'maskpass1' ROLE readonly"),
            )
            .await;
        // What Rerun sends: the history entry, with its password masked.
        let recalled = masked(&format!("ALTER USER {user} SET PASSWORD 'other'"));
        assert!(recalled.contains("'********'"), "{recalled}");
        let refused = svc.console_run(Protocol::Sql, "", &recalled).await;
        assert!(matches!(refused, Err(StudioError::Setup(_))), "{refused:?}");
        let refused = svc
            .run_query(&QueryRequest {
                sql: recalled,
                params: Vec::new(),
                database: String::new(),
                page: 0,
                page_size: 10,
            })
            .await;
        assert!(refused.is_err());
        // The password is the one set at creation.
        let (_, session) = live_as(user, "maskpass1").await;
        assert_eq!(session.user, user);
    }

    #[tokio::test]
    #[ignore = "needs a live NodeDB 0.6.2 server: see `live`"]
    async fn live_console_runs_sql_admin_and_cypher_and_keeps_history() {
        let svc = live().await;
        let c = svc.client(None).expect("a client");
        let t = "studio_probe_w_graph";
        for sql in [
            format!("CREATE COLLECTION IF NOT EXISTS {t}"),
            format!("UPSERT INTO {t} (id) VALUES ('ga'), ('gb')"),
            format!("GRAPH INSERT EDGE IN '{t}' FROM 'ga' TO 'gb' TYPE 'studio_w_knows'"),
        ] {
            c.query(&sql).await.expect("seed runs");
        }
        let sql = svc
            .console_run(Protocol::Sql, "", "SELECT 1 AS one;")
            .await
            .expect("runs");
        assert_eq!(
            sql.output,
            ConsoleOutput::Table {
                columns: vec!["one".into()],
                rows: vec![vec!["1".into()]],
            }
        );
        assert_eq!(sql.tx, TxState::Idle);
        let admin = svc
            .console_run(Protocol::Admin, "", "SHOW DATABASES")
            .await
            .expect("runs");
        let ConsoleOutput::Table { rows, .. } = admin.output else {
            panic!("SHOW DATABASES is a table");
        };
        assert!(
            rows.iter()
                .any(|r| r.first().is_some_and(|c| c == "default"))
        );
        let cypher = svc
            .console_run(
                Protocol::Cypher,
                "",
                "MATCH (a)-[:studio_w_knows]->(b) RETURN a, b",
            )
            .await
            .expect("runs");
        let ConsoleOutput::Table { columns, rows } = cypher.output else {
            panic!("MATCH is a table");
        };
        assert_eq!(columns, ["a", "b"]);
        assert!(rows.contains(&vec!["ga".to_string(), "gb".to_string()]));
        assert!(svc.console_run(Protocol::Sql, "", "SELEC 1").await.is_err());
        let all = svc.console_history(&history(None)).await.expect("reads");
        let texts: Vec<&str> = all.iter().map(|h| h.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "SELEC 1",
                "MATCH (a)-[:studio_w_knows]->(b) RETURN a, b",
                "SHOW DATABASES",
                "SELECT 1 AS one;",
            ]
        );
        let admin_only = svc
            .console_history(&history(Some(Protocol::Admin)))
            .await
            .expect("reads");
        assert_eq!(admin_only.len(), 1);
    }

    #[tokio::test]
    #[ignore = "needs a live NodeDB 0.6.2 server: see `live`"]
    async fn live_query_explain_returns_the_server_plan_and_never_runs() {
        let svc = live().await;
        let c = svc.client(None).expect("a client");
        let t = "studio_probe_w_plan";
        for sql in [
            format!("CREATE COLLECTION IF NOT EXISTS {t}"),
            format!("DELETE FROM {t}"),
        ] {
            c.query(&sql).await.expect("seed runs");
        }
        let mut request = q(&format!("SELECT id FROM {t} WHERE n > :n;"), 0, 50);
        request.params = vec![ParamBinding::new("n", ParamType::Integer, "1")];
        let plan = svc
            .explain(&request, ExplainMode::Explain)
            .await
            .expect("0.6.2 plans as text");
        let text = plan.text.expect("a text plan");
        assert!(text.contains("Scan") && text.contains(t), "{text}");
        assert!(plan.operators.is_empty() && plan.elapsed_ms.is_none());
        let insert = q(&format!("INSERT INTO {t} (id, n) VALUES ('x', 1)"), 0, 50);
        assert!(matches!(
            svc.explain(&insert, ExplainMode::Analyze).await,
            Err(StudioError::Setup(_))
        ));
        let planned = svc
            .explain(&insert, ExplainMode::Explain)
            .await
            .expect("an INSERT plans");
        assert!(planned.text.is_some());
        let rows = c
            .query(&format!("SELECT id FROM {t}"))
            .await
            .expect("reads");
        assert!(rows.rows.is_empty(), "neither mode ran the INSERT");
        let bad = svc
            .explain(&q("SELEC 1", 0, 50), ExplainMode::Explain)
            .await
            .expect_err("does not parse");
        assert!(bad.to_string().contains("parse error"), "{bad}");
    }

    #[tokio::test]
    #[ignore = "needs a live NodeDB 0.6.2 server: see `live`"]
    async fn live_query_schema_tree_lists_tables_fields_and_views() {
        let svc = live().await;
        let c = svc.client(None).expect("a client");
        let (strict, src, mv) = (
            "studio_probe_w_schema",
            "studio_probe_w_src",
            "studio_probe_w_mv",
        );
        for sql in [
            format!(
                "CREATE COLLECTION IF NOT EXISTS {strict} (id TEXT PRIMARY KEY, qty BIGINT NOT NULL) WITH (engine='document_strict')"
            ),
            format!("CREATE COLLECTION IF NOT EXISTS {src}"),
        ] {
            c.query(&sql).await.expect("seed runs");
        }
        // 0.6.2 answers `DROP MATERIALIZED VIEW IF EXISTS` and keeps the view,
        // so the plain DROP runs, and fails when there is no view yet.
        let _ = c.query(&format!("DROP MATERIALIZED VIEW {mv}")).await;
        c.query(&format!(
            "CREATE MATERIALIZED VIEW {mv} ON {src} AS SELECT id FROM {src}"
        ))
        .await
        .expect("the view is created");
        // Other live tests drop and create collections meanwhile. A
        // collection that vanishes inside the catalog read fails it.
        let mut tree = None;
        for _ in 0..3 {
            match svc.schema_tree().await {
                Ok(t) => {
                    tree = Some(t);
                    break;
                }
                Err(StudioError::NotFound(_)) => continue,
                Err(e) => panic!("schema tree: {e}"),
            }
        }
        let tree = tree.expect("a schema tree within three reads");
        let table = tree
            .tables
            .iter()
            .find(|t| t.name == strict)
            .expect("the strict collection");
        assert_eq!(table.kind.letter(), "S");
        assert_eq!(table.rows, None);
        let fields: Vec<(&str, &str, bool)> = table
            .fields
            .iter()
            .map(|f| (f.name.as_str(), f.type_name.as_str(), f.key))
            .collect();
        assert_eq!(fields, [("id", "string", true), ("qty", "int64", false)]);
        let view = tree
            .views
            .iter()
            .find(|v| v.name == mv)
            .expect("the materialized view");
        assert_eq!(view.kind, SchemaKind::MaterializedView);
        assert!(tree.tables.iter().all(|t| t.name != mv));
        let mut names: Vec<&str> = tree
            .tables
            .iter()
            .chain(&tree.views)
            .map(|t| t.name.as_str())
            .collect();
        let n = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), n, "every name once");
    }
}
