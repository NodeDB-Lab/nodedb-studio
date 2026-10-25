//! Graph reads: Cypher, expansion, paths, and GraphRAG retrieval.

use async_trait::async_trait;

use crate::data::mock;
use crate::models::graph::{GraphBinding, GraphResult, NodeKind, PathSet};
use crate::models::graph_rag::{RagRequest, RagResult, RagSetup};
use crate::services::error::StudioError;
use crate::services::graph_data::GraphData;
use crate::services::mock::MockConnectionService;
use crate::services::mock_behavior::{MockBehavior, apply, apply_one_result};

impl MockConnectionService {
    /// `Empty` keeps an error the fixture raises and empties a success, so a
    /// graph with no matches is reachable on every graph read.
    fn emptied<T: Default>(&self, read: Result<T, StudioError>) -> Result<T, StudioError> {
        match self.behavior {
            MockBehavior::Empty => read.map(|_| T::default()),
            MockBehavior::Ready | MockBehavior::Erroring | MockBehavior::Delayed(_) => read,
        }
    }
}

#[async_trait(?Send)]
impl GraphData for MockConnectionService {
    async fn graph_bindings(&self) -> Result<Vec<GraphBinding>, StudioError> {
        apply(self.behavior, mock::graph_bindings).await
    }

    async fn run_cypher(
        &self,
        binding: &str,
        query: &str,
        limit: usize,
    ) -> Result<GraphResult, StudioError> {
        let read = self.emptied(mock::run_cypher(binding, query, limit));
        apply_one_result(self.behavior, read).await
    }

    async fn expand_node(
        &self,
        binding: &str,
        path: &str,
        limit: usize,
    ) -> Result<GraphResult, StudioError> {
        let read = self.emptied(mock::expand_node(binding, path, limit));
        apply_one_result(self.behavior, read).await
    }

    async fn find_paths(
        &self,
        binding: &str,
        from: &str,
        to: NodeKind,
        max_hops: usize,
    ) -> Result<PathSet, StudioError> {
        let read = self.emptied(mock::find_paths(binding, from, to, max_hops));
        apply_one_result(self.behavior, read).await
    }

    async fn rag_setup(&self) -> Result<RagSetup, StudioError> {
        apply_one_result(self.behavior, Ok(mock::rag_setup())).await
    }

    async fn rag_search(&self, request: &RagRequest) -> Result<RagResult, StudioError> {
        let read = self.emptied(mock::rag_search(request));
        apply_one_result(self.behavior, read).await
    }
}
