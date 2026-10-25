//! Specialist viewer reads.

use async_trait::async_trait;

use crate::models::viewers::{FtsHit, SyncPeer};
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;
use crate::services::viewers_data::ViewersData;

#[async_trait(?Send)]
impl ViewersData for NodeDbConnectionService {
    async fn fts_hits(&self, _collection: &str, _query: &str) -> Result<Vec<FtsHit>, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn sync_peers(&self) -> Result<Vec<SyncPeer>, StudioError> {
        Err(StudioError::NotConnected)
    }
}
