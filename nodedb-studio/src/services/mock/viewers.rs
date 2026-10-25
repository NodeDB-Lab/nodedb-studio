//! Specialist viewer reads.

use async_trait::async_trait;

use crate::data::mock;
use crate::models::viewers::{
    FtsHit, SeriesPoint, SpatialFeature, SubGraph, SyncPeer, VectorPoint,
};
use crate::services::error::StudioError;
use crate::services::mock::MockConnectionService;
use crate::services::mock_behavior::{apply, apply_one_or_empty};
use crate::services::viewers_data::ViewersData;

#[async_trait(?Send)]
impl ViewersData for MockConnectionService {
    async fn sub_graph(&self, collection: &str) -> Result<SubGraph, StudioError> {
        let c = collection.to_string();
        apply_one_or_empty(
            self.behavior,
            move || mock::sub_graph(&c),
            mock::empty_sub_graph,
        )
        .await
    }

    async fn vector_points(&self, collection: &str) -> Result<Vec<VectorPoint>, StudioError> {
        let c = collection.to_string();
        apply(self.behavior, move || mock::vector_points(&c)).await
    }

    async fn series(&self, metric: &str) -> Result<Vec<SeriesPoint>, StudioError> {
        let m = metric.to_string();
        apply(self.behavior, move || mock::series(&m)).await
    }

    async fn spatial_features(&self, collection: &str) -> Result<Vec<SpatialFeature>, StudioError> {
        let c = collection.to_string();
        apply(self.behavior, move || mock::spatial_features(&c)).await
    }

    async fn fts_hits(&self, collection: &str, query: &str) -> Result<Vec<FtsHit>, StudioError> {
        let c = collection.to_string();
        let q = query.to_string();
        apply(self.behavior, move || mock::fts_hits(&c, &q)).await
    }

    async fn sync_peers(&self) -> Result<Vec<SyncPeer>, StudioError> {
        apply(self.behavior, mock::sync_peers).await
    }
}
