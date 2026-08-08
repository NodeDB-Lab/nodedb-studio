//! Workbench-tier reads at the backend seam: query execution, EXPLAIN, and the
//! schema tree.
//!
//! `run_query` returns a `ResultSet` rather than a raw table because
//! pagination is the seam's job: the real client buffers whole result sets
//! with no cursor, so the eventual implementation emits LIMIT/OFFSET here
//! rather than holding one.

use async_trait::async_trait;

use crate::models::workbench::{QueryPlan, ResultSet, SchemaNode};
use crate::services::error::StudioError;

#[async_trait(?Send)]
pub trait WorkbenchData {
    /// Execute `sql` and return one page of results.
    #[allow(dead_code)] // SEAM-UNWIRED(task-10)
    async fn run_query(&self, sql: &str) -> Result<ResultSet, StudioError>;

    /// The query planner's EXPLAIN output for `sql`.
    #[allow(dead_code)] // SEAM-UNWIRED(task-10)
    async fn explain(&self, sql: &str) -> Result<QueryPlan, StudioError>;

    /// The schema tree for the connected database.
    #[allow(dead_code)] // SEAM-UNWIRED(task-10)
    async fn schema_tree(&self) -> Result<Vec<SchemaNode>, StudioError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::async_state::AsyncState;
    use crate::services::connection_service::MockConnectionService;

    #[tokio::test]
    async fn run_query_returns_columns_and_keyed_rows() {
        let svc = MockConnectionService::ready();
        let rs = svc.run_query("SELECT 1").await.expect("query runs");
        assert!(!rs.columns.is_empty());
        for r in &rs.rows {
            assert_eq!(
                r.cells.len(),
                rs.columns.len(),
                "row width must match header"
            );
            assert!(!r.id.is_empty(), "rows need a stable key");
        }
    }

    #[tokio::test]
    async fn run_query_ready_fixture_has_three_columns_and_four_rows() {
        // Expectation authored independently of the fixture body: the brief
        // pins the shape at "3-column, 4-row", so this asserts those literal
        // numbers rather than deriving them from the call under test.
        let svc = MockConnectionService::ready();
        let rs = svc.run_query("SELECT 1").await.expect("query runs");
        assert_eq!(rs.columns.len(), 3, "fixture is documented as 3 columns");
        assert_eq!(rs.rows.len(), 4, "fixture is documented as 4 rows");
    }

    #[tokio::test]
    async fn run_query_empty_behaviour_still_returns_a_result_set() {
        // `run_query` reads a single result set, not a list: "no rows" has no
        // meaning at this call boundary, so `MockBehavior::Empty` folds into
        // the same success path as `Ready`, mirroring `record_detail`.
        let svc = MockConnectionService::empty();
        let rs = svc
            .run_query("SELECT 1")
            .await
            .expect("empty behaviour still returns a result set for a single-value read");
        assert!(!rs.columns.is_empty());
    }

    #[tokio::test]
    async fn erroring_surfaces_through_run_query() {
        let svc = MockConnectionService::erroring();
        assert!(svc.run_query("SELECT 1").await.is_err());
    }

    #[tokio::test]
    async fn explain_returns_a_plan() {
        let svc = MockConnectionService::ready();
        let p = svc.explain("SELECT 1").await.expect("explain runs");
        assert!(!p.text.is_empty());
    }

    #[tokio::test]
    async fn explain_ready_fixture_mentions_a_scan() {
        // Independently-authored expectation: the fixture is documented as a
        // deterministic plan string, so this checks for a marker the fixture
        // is known to contain rather than re-deriving it from the same call.
        let svc = MockConnectionService::ready();
        let p = svc.explain("SELECT 1").await.expect("explain runs");
        assert!(
            p.text.contains("Scan"),
            "fixture plan text must describe a scan"
        );
    }

    #[tokio::test]
    async fn explain_empty_behaviour_still_returns_a_plan() {
        let svc = MockConnectionService::empty();
        let p = svc
            .explain("SELECT 1")
            .await
            .expect("empty behaviour still returns a plan for a single-value read");
        assert!(!p.text.is_empty());
    }

    #[tokio::test]
    async fn explain_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        assert!(svc.explain("SELECT 1").await.is_err());
    }

    #[tokio::test]
    async fn schema_tree_nodes_have_unique_ids() {
        let svc = MockConnectionService::ready();
        let tree = svc.schema_tree().await.expect("schema");
        let mut ids = Vec::new();
        fn walk<'a>(ns: &'a [SchemaNode], out: &mut Vec<&'a str>) {
            for n in ns {
                out.push(n.id.as_str());
                walk(&n.children, out);
            }
        }
        walk(&tree, &mut ids);
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(total, ids.len(), "schema node ids must be unique");
    }

    #[tokio::test]
    async fn schema_tree_ready_fixture_is_at_least_two_levels_deep_with_path_like_ids() {
        // Expectation authored independently: the brief requires a fixture
        // that is structurally (not accidentally) unique, rooted at "db"
        // with a "db/users" descendant.
        let svc = MockConnectionService::ready();
        let tree = svc.schema_tree().await.expect("schema");
        let root = tree.first().expect("schema tree has a root");
        assert_eq!(root.id, "db");
        assert!(
            root.children.iter().any(|c| c.id == "db/users"),
            "root must have a db/users child"
        );
        let users = root
            .children
            .iter()
            .find(|c| c.id == "db/users")
            .expect("db/users child exists");
        assert!(
            !users.children.is_empty(),
            "db/users must have its own children for a two-level tree"
        );
    }

    #[tokio::test]
    async fn schema_tree_empty_is_empty() {
        let svc = MockConnectionService::empty();
        let s = AsyncState::from_value(Some(svc.schema_tree().await));
        assert!(s.is_empty());
    }

    #[tokio::test]
    async fn schema_tree_erroring_is_err() {
        let svc = MockConnectionService::erroring();
        let s = AsyncState::from_value(Some(svc.schema_tree().await));
        assert!(s.error_message().is_some());
    }
}
