//! Query workbench. Every method waits on the real client.

use async_trait::async_trait;

use crate::models::console::{ConsoleReply, HistoryEntry, HistoryQuery, Protocol};
use crate::models::query_plan::{ExplainMode, QueryPlan};
use crate::models::saved_query::{QueryLibrary, SaveQuery, SavedQuery, SavedQueryVersion};
use crate::models::workbench::{QueryDraft, QueryRequest, QueryRun, SchemaTree, TxAccess, TxState};
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;
use crate::services::workbench_data::WorkbenchData;

#[async_trait(?Send)]
impl WorkbenchData for NodeDbConnectionService {
    async fn run_query(&self, _request: &QueryRequest) -> Result<QueryRun, StudioError> {
        Err(StudioError::NotConnected)
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
