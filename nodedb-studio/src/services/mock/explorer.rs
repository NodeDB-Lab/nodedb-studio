//! Collection catalog and record reads.

use async_trait::async_trait;

use crate::data::mock;
use crate::models::explorer::{CollectionGroup, RecordDetail, RecordRow};
use crate::services::error::StudioError;
use crate::services::explorer_data::ExplorerData;
use crate::services::mock::MockConnectionService;
use crate::services::mock_behavior::{apply, apply_one};

#[async_trait(?Send)]
impl ExplorerData for MockConnectionService {
    async fn collection_groups(&self) -> Result<Vec<CollectionGroup>, StudioError> {
        apply(self.behavior, mock::collection_groups).await
    }

    async fn records(&self, collection: &str) -> Result<Vec<RecordRow>, StudioError> {
        let c = collection.to_string();
        apply(self.behavior, move || mock::records(&c)).await
    }

    async fn record_detail(&self, collection: &str, id: &str) -> Result<RecordDetail, StudioError> {
        let c = collection.to_string();
        let i = id.to_string();
        apply_one(self.behavior, move || mock::record_detail(&c, &i)).await
    }
}
