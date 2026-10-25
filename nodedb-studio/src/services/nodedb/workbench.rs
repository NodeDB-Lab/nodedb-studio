//! Query workbench reads.

use async_trait::async_trait;

use crate::models::workbench::{QueryPlan, ResultSet, SchemaNode};
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;
use crate::services::workbench_data::WorkbenchData;

#[async_trait(?Send)]
impl WorkbenchData for NodeDbConnectionService {
    async fn run_query(&self, _sql: &str) -> Result<ResultSet, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn explain(&self, _sql: &str) -> Result<QueryPlan, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn schema_tree(&self) -> Result<Vec<SchemaNode>, StudioError> {
        Err(StudioError::NotConnected)
    }
}
