//! Vector Space reads. The real client does not expose them yet.

use async_trait::async_trait;

use crate::models::vector::{NeighborRequest, NeighborResult, VectorRecord, VectorSetup};
use crate::models::vector_index::IndexStatus;
use crate::models::vector_projection::{Projection, ProjectionRequest};
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;
use crate::services::vector_data::VectorData;

#[async_trait(?Send)]
impl VectorData for NodeDbConnectionService {
    async fn vector_setup(&self) -> Result<VectorSetup, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn vector_record(
        &self,
        _collection: &str,
        _field: &str,
        _id: &str,
    ) -> Result<VectorRecord, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn vector_search(
        &self,
        _request: &NeighborRequest,
    ) -> Result<NeighborResult, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn vector_project(
        &self,
        _request: &ProjectionRequest,
    ) -> Result<Projection, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn vector_index_status(
        &self,
        _collection: &str,
        _field: &str,
    ) -> Result<IndexStatus, StudioError> {
        Err(StudioError::NotConnected)
    }
}
