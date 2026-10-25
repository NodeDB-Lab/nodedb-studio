//! Workbench reads and writes at the backend seam: running and explaining
//! statements, the session's transaction, saved queries, and the Console.
//!
//! `run_query` returns a `ResultSet` page rather than a raw table because
//! pagination is the seam's job: the real client buffers whole result sets
//! with no cursor, so the eventual implementation emits LIMIT/OFFSET here
//! rather than holding one.
//!
//! The transaction belongs to the session: the Query page and the Console
//! run in the same one, and either can commit it.

use async_trait::async_trait;

use crate::models::console::{ConsoleReply, HistoryEntry, HistoryQuery, Protocol};
use crate::models::query_plan::{ExplainMode, QueryPlan};
use crate::models::saved_query::{QueryLibrary, SaveQuery, SavedQuery, SavedQueryVersion};
use crate::models::workbench::{QueryDraft, QueryRequest, QueryRun, SchemaTree, TxAccess, TxState};
use crate::services::error::StudioError;

#[async_trait(?Send)]
pub trait WorkbenchData {
    /// Execute a statement and return one page of its rows. A statement that
    /// fails inside an open transaction fails the transaction.
    async fn run_query(&self, request: &QueryRequest) -> Result<QueryRun, StudioError>;

    /// The plan of a statement. `Explain` plans only: the statement does not
    /// run and the plan carries estimates. `Analyze` runs it and measures.
    async fn explain(
        &self,
        request: &QueryRequest,
        mode: ExplainMode,
    ) -> Result<QueryPlan, StudioError>;

    /// The tables, views, and fields of the connected database.
    async fn schema_tree(&self) -> Result<SchemaTree, StudioError>;

    /// The query tabs the session restores.
    async fn query_session(&self) -> Result<Vec<QueryDraft>, StudioError>;

    /// The session's transaction.
    async fn transaction(&self) -> Result<TxState, StudioError>;

    /// Open a transaction. `StudioError::TransactionOpen` when one is open.
    async fn begin_transaction(&self, access: TxAccess) -> Result<TxState, StudioError>;

    /// Commit the open transaction. `StudioError::NoTransaction` when none
    /// is open. A failed transaction rolls back instead.
    async fn commit_transaction(&self) -> Result<TxState, StudioError>;

    /// Roll the open transaction back.
    async fn rollback_transaction(&self) -> Result<TxState, StudioError>;

    /// Every folder and saved query the session can open.
    async fn query_library(&self) -> Result<QueryLibrary, StudioError>;

    /// The stored versions of a saved query, newest first.
    async fn saved_query_versions(&self, id: &str) -> Result<Vec<SavedQueryVersion>, StudioError>;

    /// Create a saved query, or store a new version of one. A name another
    /// query holds is `StudioError::QueryNameTaken`.
    async fn save_query(&self, save: &SaveQuery) -> Result<SavedQuery, StudioError>;

    /// Remove a saved query and its versions.
    async fn delete_saved_query(&self, id: &str) -> Result<(), StudioError>;

    /// Add a folder to My queries.
    async fn create_query_folder(&self, name: &str) -> Result<(), StudioError>;

    /// Run one console command in `protocol` against `database`.
    async fn console_run(
        &self,
        protocol: Protocol,
        database: &str,
        text: &str,
    ) -> Result<ConsoleReply, StudioError>;

    /// The commands the session ran, newest first.
    async fn console_history(&self, query: &HistoryQuery)
    -> Result<Vec<HistoryEntry>, StudioError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::console::{ConsoleOutput, HistoryWindow};
    use crate::models::saved_query::QueryScope;
    use crate::models::workbench::{ParamBinding, ParamType};
    use crate::services::async_state::AsyncState;
    use crate::services::mock::MockConnectionService;

    fn request(sql: &str) -> QueryRequest {
        QueryRequest {
            sql: sql.to_string(),
            params: Vec::new(),
            database: "analytics".to_string(),
            page: 0,
            page_size: 50,
        }
    }

    const EVENTS: &str = "SELECT type, COUNT(*) AS n FROM events GROUP BY type ORDER BY n DESC;";
    const ORDERS: &str =
        "SELECT customer_id, SUM(total) FROM orders WHERE currency = 'USD' GROUP BY customer_id";

    fn spend(page: usize, size: usize) -> QueryRequest {
        QueryRequest {
            sql: "SELECT c.name FROM customers c WHERE o.currency = :currency".to_string(),
            params: vec![ParamBinding::new("currency", ParamType::Text, "USD")],
            page,
            page_size: size,
            ..request("")
        }
    }

    // --- run_query

    #[tokio::test]
    async fn run_query_returns_columns_and_keyed_rows() {
        let svc = MockConnectionService::ready();
        let run = svc.run_query(&request(EVENTS)).await.expect("query runs");
        let rs = run.result;
        assert!(!rs.columns().is_empty());
        for r in rs.rows() {
            assert_eq!(
                r.cells.len(),
                rs.columns().len(),
                "row width matches header"
            );
            assert!(!r.id.is_empty(), "rows need a stable key");
        }
    }

    #[tokio::test]
    async fn the_events_fixture_is_seven_rows_of_three_columns() {
        // Pinned to the design's numbers, not derived from the fixture.
        let svc = MockConnectionService::ready();
        let rs = svc.run_query(&request(EVENTS)).await.expect("runs").result;
        assert_eq!(rs.columns(), ["type", "n", "avg_load"]);
        assert_eq!(rs.rows().len(), 7);
        assert_eq!((rs.elapsed_ms, rs.scanned.as_str()), (142, "14M"));
        assert_eq!(rs.access_path, "index_scan(events_ts_idx)");
    }

    #[tokio::test]
    async fn a_paged_result_reports_its_total() {
        let svc = MockConnectionService::ready();
        let rs = svc.run_query(&spend(0, 5)).await.expect("runs").result;
        assert_eq!((rs.rows().len(), rs.total_rows, rs.pages()), (5, 127, 26));
        assert_eq!(rs.rows()[0].cells[0], "C1001");
        let last = svc.run_query(&spend(25, 5)).await.expect("runs").result;
        assert_eq!(last.rows().len(), 2, "127 rows leave 2 on page 26");
        assert_eq!(last.shown(), Some((126, 127)));
    }

    #[tokio::test]
    async fn row_ids_are_unique_on_every_page() {
        let svc = MockConnectionService::ready();
        for page in [0, 3, 25] {
            let rs = svc.run_query(&spend(page, 5)).await.expect("runs").result;
            let mut ids: Vec<&str> = rs.rows().iter().map(|r| r.id.as_str()).collect();
            let n = ids.len();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(ids.len(), n);
        }
    }

    #[tokio::test]
    async fn an_unbound_parameter_is_refused() {
        let svc = MockConnectionService::ready();
        let mut r = spend(0, 5);
        r.params.clear();
        assert!(matches!(
            svc.run_query(&r).await,
            Err(StudioError::MissingParameter(p)) if p == "currency"
        ));
    }

    #[tokio::test]
    async fn an_empty_or_unbalanced_statement_is_refused() {
        let svc = MockConnectionService::ready();
        assert!(matches!(
            svc.run_query(&request("  ")).await,
            Err(StudioError::EmptyStatement)
        ));
        assert!(matches!(
            svc.run_query(&request("SELECT (1")).await,
            Err(StudioError::Syntax { .. })
        ));
        assert!(matches!(
            svc.run_query(&request("SELECT 'a")).await,
            Err(StudioError::Syntax { .. })
        ));
    }

    #[tokio::test]
    async fn run_query_empty_behaviour_returns_zero_rows_with_columns() {
        let svc = MockConnectionService::empty();
        let rs = svc.run_query(&request(EVENTS)).await.expect("ok").result;
        assert!(rs.rows().is_empty());
        assert_eq!(rs.columns().len(), 3, "an empty result keeps its header");
    }

    #[tokio::test]
    async fn erroring_surfaces_through_every_read_and_write() {
        let svc = MockConnectionService::erroring();
        assert!(svc.run_query(&request(EVENTS)).await.is_err());
        assert!(
            svc.explain(&request(EVENTS), ExplainMode::Explain)
                .await
                .is_err()
        );
        assert!(svc.schema_tree().await.is_err());
        assert!(svc.transaction().await.is_err());
        assert!(svc.query_library().await.is_err());
        assert!(svc.begin_transaction(TxAccess::ReadOnly).await.is_err());
        assert!(
            svc.console_history(&history(None, HistoryWindow::AllTime))
                .await
                .is_err()
        );
    }

    // --- explain

    #[tokio::test]
    async fn explain_carries_estimates_and_no_measurement() {
        let svc = MockConnectionService::ready();
        let plan = svc
            .explain(&request(ORDERS), ExplainMode::Explain)
            .await
            .expect("plans");
        assert_eq!(plan.mode, ExplainMode::Explain);
        assert!(plan.elapsed_ms.is_none());
        for op in &plan.operators {
            assert!(op.actual_rows.is_none(), "Explain never measures rows");
            assert!(op.time_us.is_none(), "Explain never measures time");
        }
    }

    #[tokio::test]
    async fn analyze_measures_every_operator() {
        let svc = MockConnectionService::ready();
        let plan = svc
            .explain(&request(ORDERS), ExplainMode::Analyze)
            .await
            .expect("plans");
        assert!(plan.elapsed_ms.is_some());
        for op in &plan.operators {
            assert!(op.actual_rows.is_some() && op.time_us.is_some());
        }
    }

    #[tokio::test]
    async fn the_orders_plan_is_the_designed_chain() {
        let svc = MockConnectionService::ready();
        let plan = svc
            .explain(&request(ORDERS), ExplainMode::Explain)
            .await
            .expect("plans");
        let names: Vec<&str> = plan.operators.iter().map(|o| o.kind.label()).collect();
        assert_eq!(
            names,
            ["Projection", "HashAggregate", "Filter", "ColumnarScan"]
        );
        let est: Vec<u64> = plan.operators.iter().map(|o| o.est_rows).collect();
        assert_eq!(&est[..3], [1_024, 1_024, 12_480]);
        let filter = plan.operator(3).expect("filter");
        assert!(filter.facts.iter().any(|(_, v)| v == "currency = 'USD'"));
    }

    #[tokio::test]
    async fn operator_ids_are_unique_and_inputs_exist() {
        let svc = MockConnectionService::ready();
        for sql in [ORDERS, EVENTS, "SELECT 1"] {
            let plan = svc
                .explain(&request(sql), ExplainMode::Explain)
                .await
                .expect("plans");
            let mut ids: Vec<u32> = plan.operators.iter().map(|o| o.id).collect();
            let n = ids.len();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(ids.len(), n, "{sql}");
            for op in &plan.operators {
                if let Some(input) = op.input {
                    assert!(plan.operator(input).is_some(), "{sql}: #{input}");
                }
            }
        }
    }

    #[tokio::test]
    async fn explain_does_not_touch_the_transaction_but_analyze_runs() {
        let svc = MockConnectionService::ready();
        svc.begin_transaction(TxAccess::ReadOnly)
            .await
            .expect("begin");
        svc.explain(&request(ORDERS), ExplainMode::Explain)
            .await
            .expect("plans");
        assert_eq!(
            statements(&svc).await,
            0,
            "Explain does not run the statement"
        );
        svc.explain(&request(ORDERS), ExplainMode::Analyze)
            .await
            .expect("plans");
        assert_eq!(statements(&svc).await, 1, "Analyze runs it");
    }

    async fn statements(svc: &MockConnectionService) -> u32 {
        match svc.transaction().await.expect("state") {
            TxState::Active { statements, .. } => statements,
            other => panic!("expected an open transaction, got {other:?}"),
        }
    }

    // --- schema

    #[tokio::test]
    async fn schema_names_are_unique_and_orders_is_strict() {
        let svc = MockConnectionService::ready();
        let tree = svc.schema_tree().await.expect("schema");
        let mut names: Vec<&str> = tree
            .tables
            .iter()
            .chain(&tree.views)
            .map(|t| t.name.as_str())
            .collect();
        let n = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), n);
        let orders = tree
            .tables
            .iter()
            .find(|t| t.name == "orders")
            .expect("orders");
        assert_eq!(orders.kind.letter(), "S");
        assert_eq!(orders.count_label(), "442,003");
        let events = tree
            .tables
            .iter()
            .find(|t| t.name == "events")
            .expect("events");
        assert_eq!(events.count_label(), "2.4M");
    }

    #[tokio::test]
    async fn schema_empty_is_empty_and_erroring_is_an_error() {
        let svc = MockConnectionService::empty();
        let s = AsyncState::from_value(Some(svc.schema_tree().await));
        assert!(s.is_empty());
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.schema_tree().await));
        assert!(s.error_message().is_some());
    }

    // --- session and transactions

    #[tokio::test]
    async fn the_session_opens_three_tabs_and_one_is_unsaved() {
        let svc = MockConnectionService::ready();
        let tabs = svc.query_session().await.expect("session");
        assert_eq!(tabs.len(), 3);
        let unsaved: Vec<&str> = tabs
            .iter()
            .filter(|t| t.sql != t.saved_sql)
            .map(|t| t.title.as_str())
            .collect();
        assert_eq!(unsaved, ["top users"]);
        let mut ids: Vec<&str> = tabs.iter().map(|t| t.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 3);
    }

    #[tokio::test]
    async fn a_transaction_begins_commits_and_rolls_back() {
        let svc = MockConnectionService::ready();
        assert_eq!(svc.transaction().await.expect("state"), TxState::Idle);
        let open = svc
            .begin_transaction(TxAccess::ReadOnly)
            .await
            .expect("begin");
        assert_eq!(open.label(), "Active (read-only)");
        assert!(matches!(
            svc.begin_transaction(TxAccess::ReadWrite).await,
            Err(StudioError::TransactionOpen)
        ));
        assert_eq!(
            svc.commit_transaction().await.expect("commit"),
            TxState::Idle
        );
        assert!(matches!(
            svc.commit_transaction().await,
            Err(StudioError::NoTransaction)
        ));
        svc.begin_transaction(TxAccess::ReadWrite)
            .await
            .expect("begin");
        assert_eq!(
            svc.rollback_transaction().await.expect("rollback"),
            TxState::Idle
        );
        assert!(matches!(
            svc.rollback_transaction().await,
            Err(StudioError::NoTransaction)
        ));
    }

    #[tokio::test]
    async fn a_write_in_a_read_only_transaction_fails_the_transaction() {
        let svc = MockConnectionService::ready();
        svc.begin_transaction(TxAccess::ReadOnly)
            .await
            .expect("begin");
        let write = request("UPDATE orders SET status = 'paid' WHERE id = 1");
        assert!(matches!(
            svc.run_query(&write).await,
            Err(StudioError::ReadOnlyTransaction)
        ));
        assert_eq!(svc.transaction().await.expect("state"), TxState::Failed);
        assert!(matches!(
            svc.run_query(&request(EVENTS)).await,
            Err(StudioError::Rejected { .. })
        ));
        assert_eq!(
            svc.rollback_transaction().await.expect("rollback"),
            TxState::Idle
        );
        assert!(svc.run_query(&request(EVENTS)).await.is_ok());
    }

    #[tokio::test]
    async fn a_write_in_a_read_write_transaction_is_counted() {
        let svc = MockConnectionService::ready();
        svc.begin_transaction(TxAccess::ReadWrite)
            .await
            .expect("begin");
        let run = svc
            .run_query(&request("INSERT INTO orders VALUES (1)"))
            .await
            .expect("write runs");
        assert_eq!(run.result.columns(), ["affected_rows"]);
        assert_eq!(statements(&svc).await, 1);
    }

    // --- saved queries

    fn save(name: &str) -> SaveQuery {
        SaveQuery {
            id: None,
            name: name.to_string(),
            folder: None,
            database: "analytics".to_string(),
            sql: "SELECT 1".to_string(),
            params: Vec::new(),
            tags: vec!["t".to_string()],
        }
    }

    #[tokio::test]
    async fn the_library_groups_match_the_design() {
        let svc = MockConnectionService::ready();
        let lib = svc.query_library().await.expect("library");
        let names = |v: Vec<&SavedQuery>| v.iter().map(|q| q.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(lib.mine()), ["Customer spend", "Sales daily"]);
        assert_eq!(
            names(lib.shared()),
            ["Product performance", "Top customers"]
        );
        assert_eq!(names(lib.recent_errors()), ["Failed orders"]);
        let mut ids: Vec<&str> = lib.queries.iter().map(|q| q.id.as_str()).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n);
    }

    #[tokio::test]
    async fn customer_spend_has_its_parameter_and_three_versions() {
        let svc = MockConnectionService::ready();
        let lib = svc.query_library().await.expect("library");
        let q = lib
            .get(crate::data::mock::CUSTOMER_SPEND_ID)
            .expect("spend");
        assert_eq!(
            (q.owner.as_str(), q.scope, q.version),
            ("Hana", QueryScope::MyWorkspace, 3)
        );
        assert_eq!(
            q.params,
            [ParamBinding::new("currency", ParamType::Text, "USD")]
        );
        assert_eq!(q.tags, ["customers", "revenue"]);
        let v = svc.saved_query_versions(&q.id).await.expect("versions");
        assert_eq!(v.iter().map(|v| v.version).collect::<Vec<_>>(), [3, 2, 1]);
        assert!(matches!(
            svc.saved_query_versions("nope").await,
            Err(StudioError::UnknownQuery(_))
        ));
    }

    #[tokio::test]
    async fn saving_creates_then_versions() {
        let svc = MockConnectionService::ready();
        let made = svc
            .save_query(&save("Weekly cohort"))
            .await
            .expect("create");
        assert_eq!((made.version, made.scope), (1, QueryScope::MyWorkspace));
        let mut again = save("Weekly cohort v2");
        again.id = Some(made.id.clone());
        let v2 = svc.save_query(&again).await.expect("update");
        assert_eq!((v2.id.as_str(), v2.version), (made.id.as_str(), 2));
        assert!(v2.updated_at > made.updated_at);
        let v = svc.saved_query_versions(&made.id).await.expect("versions");
        assert_eq!(v.len(), 2);
        let lib = svc.query_library().await.expect("library");
        assert_eq!(
            lib.get(&made.id).map(|q| q.name.as_str()),
            Some("Weekly cohort v2")
        );
    }

    #[tokio::test]
    async fn a_name_is_unique_and_required() {
        let svc = MockConnectionService::ready();
        assert!(matches!(
            svc.save_query(&save("customer SPEND")).await,
            Err(StudioError::QueryNameTaken(_))
        ));
        assert!(matches!(
            svc.save_query(&save("  ")).await,
            Err(StudioError::Rejected { .. })
        ));
        let mut rename = save("Customer spend");
        rename.id = Some("q-customer-spend".to_string());
        assert!(
            svc.save_query(&rename).await.is_ok(),
            "a query keeps its own name"
        );
    }

    #[tokio::test]
    async fn only_the_owner_saves_a_shared_query() {
        let svc = MockConnectionService::ready();
        let mut s = save("Product performance");
        s.id = Some("q-product-performance".to_string());
        assert!(matches!(
            svc.save_query(&s).await,
            Err(StudioError::Rejected { .. })
        ));
    }

    #[tokio::test]
    async fn folders_hold_queries_and_deleting_removes_the_query() {
        let svc = MockConnectionService::ready();
        svc.create_query_folder("Finance").await.expect("folder");
        assert!(svc.create_query_folder("finance").await.is_err());
        let mut s = save("Ledger");
        s.folder = Some("Finance".to_string());
        let made = svc.save_query(&s).await.expect("create");
        let lib = svc.query_library().await.expect("library");
        assert_eq!(lib.in_folder("Finance").len(), 1);
        assert!(lib.mine().iter().all(|q| q.id != made.id));
        let mut bad = save("Lost");
        bad.folder = Some("Missing".to_string());
        assert!(svc.save_query(&bad).await.is_err());
        svc.delete_saved_query(&made.id).await.expect("delete");
        assert!(matches!(
            svc.delete_saved_query(&made.id).await,
            Err(StudioError::UnknownQuery(_))
        ));
    }

    #[tokio::test]
    async fn an_empty_library_is_empty() {
        let svc = MockConnectionService::empty();
        let s = AsyncState::from_value(Some(svc.query_library().await));
        assert!(s.is_empty());
    }

    // --- console

    fn history(protocol: Option<Protocol>, window: HistoryWindow) -> HistoryQuery {
        HistoryQuery { protocol, window }
    }

    #[tokio::test]
    async fn sql_history_lists_the_designed_eight_newest_first() {
        let svc = MockConnectionService::ready();
        let h = svc
            .console_history(&history(Some(Protocol::Sql), HistoryWindow::AllTime))
            .await
            .expect("history");
        assert_eq!(h.len(), 8);
        assert_eq!(h[0].text, "SELECT current_database();");
        assert_eq!(h[7].text, "SELECT name FROM databases;");
        assert!(h.windows(2).all(|w| w[0].at >= w[1].at));
        let all = svc
            .console_history(&history(None, HistoryWindow::AllTime))
            .await
            .expect("history");
        assert!(all.len() > h.len());
    }

    #[tokio::test]
    async fn a_window_drops_older_commands() {
        let svc = MockConnectionService::ready();
        let day = svc
            .console_history(&history(Some(Protocol::Sql), HistoryWindow::Day))
            .await
            .expect("history");
        let week = svc
            .console_history(&history(Some(Protocol::Sql), HistoryWindow::Week))
            .await
            .expect("history");
        assert_eq!(day.len(), 8, "every SQL fixture is within a day");
        assert_eq!(week.len(), 8);
        let admin_day = svc
            .console_history(&history(Some(Protocol::Admin), HistoryWindow::Day))
            .await
            .expect("history");
        let admin_all = svc
            .console_history(&history(Some(Protocol::Admin), HistoryWindow::AllTime))
            .await
            .expect("history");
        assert!(admin_day.len() < admin_all.len());
    }

    #[tokio::test]
    async fn a_command_runs_and_lands_in_history() {
        let svc = MockConnectionService::ready();
        let reply = svc
            .console_run(Protocol::Sql, "analytics", "SELECT count(*) FROM users;")
            .await
            .expect("runs");
        assert_eq!(reply.elapsed_ms, 18);
        assert_eq!(
            reply.output,
            ConsoleOutput::Table {
                columns: vec!["count".into()],
                rows: vec![vec!["1284".into()]],
            }
        );
        assert_eq!(reply.tx, TxState::Idle);
        let h = svc
            .console_history(&history(Some(Protocol::Sql), HistoryWindow::AllTime))
            .await
            .expect("history");
        assert_eq!(h[0].text, "SELECT count(*) FROM users;");
        assert_eq!(h.len(), 9);
    }

    #[tokio::test]
    async fn the_target_database_reaches_the_reply() {
        let svc = MockConnectionService::ready();
        let reply = svc
            .console_run(Protocol::Sql, "staging", "SELECT current_database();")
            .await
            .expect("runs");
        assert_eq!(
            reply.output,
            ConsoleOutput::Table {
                columns: vec!["current_database".into()],
                rows: vec![vec!["staging".into()]],
            }
        );
    }

    #[tokio::test]
    async fn console_transaction_control_moves_the_shared_transaction() {
        let svc = MockConnectionService::ready();
        let begun = svc
            .console_run(Protocol::Sql, "analytics", "BEGIN;")
            .await
            .expect("begin");
        assert_eq!(begun.tx.label(), "Active (read-write)");
        assert_eq!(svc.transaction().await.expect("state"), begun.tx);
        let done = svc
            .console_run(Protocol::Sql, "analytics", "rollback")
            .await
            .expect("rollback");
        assert_eq!(done.tx, TxState::Idle);
        assert!(
            svc.console_run(Protocol::Sql, "analytics", "COMMIT")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn each_protocol_rejects_what_it_cannot_parse() {
        let svc = MockConnectionService::ready();
        assert!(matches!(
            svc.console_run(Protocol::Sql, "analytics", " ; ").await,
            Err(StudioError::EmptyStatement)
        ));
        assert!(matches!(
            svc.console_run(Protocol::Cypher, "analytics", "SELECT 1")
                .await,
            Err(StudioError::Syntax { .. })
        ));
        assert!(
            svc.console_run(Protocol::Cypher, "analytics", "MATCH (u:User) RETURN u")
                .await
                .is_ok()
        );
        assert!(matches!(
            svc.console_run(Protocol::Admin, "analytics", "FLY").await,
            Err(StudioError::Syntax { .. })
        ));
        let status = svc
            .console_run(Protocol::Admin, "analytics", "SHOW CLUSTER STATUS")
            .await
            .expect("status");
        let ConsoleOutput::Text(lines) = status.output else {
            panic!("a status report is text");
        };
        assert!(
            lines.iter().all(|l| !l.contains("leader: node")),
            "a cluster has one leader per group, never one"
        );
    }
}
