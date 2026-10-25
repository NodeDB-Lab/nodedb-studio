//! Query workbench reads.

use async_trait::async_trait;

use crate::data::mock;
use crate::models::workbench::{QueryPlan, ResultSet, SchemaNode};
use crate::services::error::StudioError;
use crate::services::mock::MockConnectionService;
use crate::services::mock_behavior::{apply, apply_one, apply_one_or_empty};
use crate::services::workbench_data::WorkbenchData;

#[async_trait(?Send)]
impl WorkbenchData for MockConnectionService {
    async fn run_query(&self, sql: &str) -> Result<ResultSet, StudioError> {
        let s = sql.to_string();
        let s2 = s.clone();
        apply_one_or_empty(
            self.behavior,
            move || mock::result_set(&s),
            move || mock::empty_result_set(&s2),
        )
        .await
    }

    async fn explain(&self, sql: &str) -> Result<QueryPlan, StudioError> {
        let s = sql.to_string();
        apply_one(self.behavior, move || mock::query_plan(&s)).await
    }

    async fn schema_tree(&self) -> Result<Vec<SchemaNode>, StudioError> {
        apply(self.behavior, mock::schema_tree).await
    }
}
