//! Query workbench: statements, plans, the transaction, saved queries, and
//! the Console. State lives in the service's `WorkbenchStore`.

use async_trait::async_trait;

use crate::data::mock;
use crate::models::console::{ConsoleReply, HistoryEntry, HistoryQuery, Protocol};
use crate::models::query_plan::{ExplainMode, QueryPlan};
use crate::models::saved_query::{QueryLibrary, SaveQuery, SavedQuery, SavedQueryVersion};
use crate::models::workbench::{QueryDraft, QueryRequest, QueryRun, SchemaTree, TxAccess, TxState};
use crate::services::error::StudioError;
use crate::services::mock::MockConnectionService;
use crate::services::mock_behavior::{MockBehavior, apply_one};
use crate::services::workbench_data::WorkbenchData;

impl MockConnectionService {
    /// Run `f` on the store after the behavior's gate: an `Erroring` mock
    /// refuses writes like it refuses reads.
    async fn wb<T>(
        &self,
        f: impl FnOnce(&mut mock::WorkbenchStore) -> Result<T, StudioError>,
    ) -> Result<T, StudioError> {
        apply_one(self.behavior, || ()).await?;
        f(&mut self.workbench.borrow_mut())
    }

    fn is_empty_mode(&self) -> bool {
        self.behavior == MockBehavior::Empty
    }
}

#[async_trait(?Send)]
impl WorkbenchData for MockConnectionService {
    async fn run_query(&self, request: &QueryRequest) -> Result<QueryRun, StudioError> {
        let mut run = self.wb(|s| s.run(request)).await?;
        if self.is_empty_mode() {
            run.result = mock::empty_result_set(request);
        }
        Ok(run)
    }

    async fn explain(
        &self,
        request: &QueryRequest,
        mode: ExplainMode,
    ) -> Result<QueryPlan, StudioError> {
        if mode == ExplainMode::Analyze {
            // Analyze runs the statement, so it obeys the transaction.
            self.wb(|s| s.run(request).map(|_| ())).await?;
        } else {
            apply_one(self.behavior, || ()).await?;
            if request.sql.trim().is_empty() {
                return Err(StudioError::EmptyStatement);
            }
        }
        Ok(mock::query_plan(&request.sql, mode))
    }

    async fn schema_tree(&self) -> Result<SchemaTree, StudioError> {
        apply_one(self.behavior, || ()).await?;
        Ok(if self.is_empty_mode() {
            SchemaTree::default()
        } else {
            mock::schema_tree()
        })
    }

    async fn query_session(&self) -> Result<Vec<QueryDraft>, StudioError> {
        apply_one(self.behavior, || ()).await?;
        Ok(if self.is_empty_mode() {
            Vec::new()
        } else {
            mock::query_session()
        })
    }

    async fn transaction(&self) -> Result<TxState, StudioError> {
        self.wb(|s| Ok(s.transaction())).await
    }

    async fn begin_transaction(&self, access: TxAccess) -> Result<TxState, StudioError> {
        self.wb(|s| s.begin(access)).await
    }

    async fn commit_transaction(&self) -> Result<TxState, StudioError> {
        self.wb(|s| s.commit()).await
    }

    async fn rollback_transaction(&self) -> Result<TxState, StudioError> {
        self.wb(|s| s.rollback()).await
    }

    async fn query_library(&self) -> Result<QueryLibrary, StudioError> {
        let lib = self.wb(|s| Ok(s.library())).await?;
        Ok(if self.is_empty_mode() {
            QueryLibrary::default()
        } else {
            lib
        })
    }

    async fn saved_query_versions(&self, id: &str) -> Result<Vec<SavedQueryVersion>, StudioError> {
        self.wb(|s| s.versions(id)).await
    }

    async fn save_query(&self, save: &SaveQuery) -> Result<SavedQuery, StudioError> {
        self.wb(|s| s.save(save)).await
    }

    async fn delete_saved_query(&self, id: &str) -> Result<(), StudioError> {
        self.wb(|s| s.delete(id)).await
    }

    async fn create_query_folder(&self, name: &str) -> Result<(), StudioError> {
        self.wb(|s| s.create_folder(name)).await
    }

    async fn console_run(
        &self,
        protocol: Protocol,
        database: &str,
        text: &str,
    ) -> Result<ConsoleReply, StudioError> {
        self.wb(|s| s.console(protocol, database, text)).await
    }

    async fn console_history(
        &self,
        query: &HistoryQuery,
    ) -> Result<Vec<HistoryEntry>, StudioError> {
        let out = self.wb(|s| Ok(s.history(query))).await?;
        Ok(if self.is_empty_mode() {
            Vec::new()
        } else {
            out
        })
    }
}
