//! Collection catalog and record reads.

use async_trait::async_trait;

use crate::models::explorer::{CollectionGroup, RecordDetail, RecordRow};
use crate::services::error::StudioError;
use crate::services::explorer_data::ExplorerData;
use crate::services::nodedb::NodeDbConnectionService;

#[async_trait(?Send)]
impl ExplorerData for NodeDbConnectionService {
    async fn collection_groups(&self) -> Result<Vec<CollectionGroup>, StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn records(&self, _collection: &str) -> Result<Vec<RecordRow>, StudioError> {
        Err(StudioError::NotConnected)
    }
    async fn record_detail(
        &self,
        _collection: &str,
        _id: &str,
    ) -> Result<RecordDetail, StudioError> {
        Err(StudioError::NotConnected)
    }
}
