//! Specialist viewer reads.

use async_trait::async_trait;

use crate::data::mock;
use crate::models::viewers::SyncPeer;
use crate::services::error::StudioError;
use crate::services::mock::MockConnectionService;
use crate::services::mock_behavior::apply;
use crate::services::viewers_data::ViewersData;

#[async_trait(?Send)]
impl ViewersData for MockConnectionService {
    async fn sync_peers(&self) -> Result<Vec<SyncPeer>, StudioError> {
        apply(self.behavior, mock::sync_peers).await
    }
}
