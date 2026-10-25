//! Graph reads. The real client does not expose them yet.

use async_trait::async_trait;

use crate::models::graph::{GraphBinding, GraphResult, NodeKind, PathSet};
use crate::models::graph_rag::{RagRequest, RagResult, RagSetup};
use crate::services::error::StudioError;
use crate::services::graph_data::GraphData;
use crate::services::nodedb::NodeDbConnectionService;

#[async_trait(?Send)]
impl GraphData for NodeDbConnectionService {
    async fn graph_bindings(&self) -> Result<Vec<GraphBinding>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn run_cypher(
        &self,
        _binding: &str,
        _query: &str,
        _limit: usize,
    ) -> Result<GraphResult, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn expand_node(
        &self,
        _binding: &str,
        _path: &str,
        _limit: usize,
    ) -> Result<GraphResult, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn find_paths(
        &self,
        _binding: &str,
        _from: &str,
        _to: NodeKind,
        _max_hops: usize,
    ) -> Result<PathSet, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn rag_setup(&self) -> Result<RagSetup, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn rag_search(&self, _request: &RagRequest) -> Result<RagResult, StudioError> {
        Err(StudioError::NotConnected)
    }
}
