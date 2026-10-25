//! Specialist viewer reads.

use async_trait::async_trait;

use crate::models::viewers::{
    FtsHit, SeriesPoint, SpatialFeature, SubGraph, SyncPeer, VectorPoint,
};
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;
use crate::services::viewers_data::ViewersData;

#[async_trait(?Send)]
impl ViewersData for NodeDbConnectionService {
    async fn sub_graph(&self, _collection: &str) -> Result<SubGraph, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn vector_points(&self, _collection: &str) -> Result<Vec<VectorPoint>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn series(&self, _metric: &str) -> Result<Vec<SeriesPoint>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn spatial_features(
        &self,
        _collection: &str,
    ) -> Result<Vec<SpatialFeature>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn fts_hits(&self, _collection: &str, _query: &str) -> Result<Vec<FtsHit>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn sync_peers(&self) -> Result<Vec<SyncPeer>, StudioError> {
        Err(StudioError::NotConnected)
    }
}
