//! Cluster and administration reads.

use async_trait::async_trait;

use crate::data::mock;
use crate::models::admin::{AuditEntry, ClusterNode, RaftGroup, RlsPolicy, ShardRange, UserRow};
use crate::services::admin_data::AdminData;
use crate::services::error::StudioError;
use crate::services::mock::MockConnectionService;
use crate::services::mock_behavior::apply;

#[async_trait(?Send)]
impl AdminData for MockConnectionService {
    async fn cluster_nodes(&self) -> Result<Vec<ClusterNode>, StudioError> {
        apply(self.behavior, mock::cluster_nodes).await
    }

    async fn raft_groups(&self) -> Result<Vec<RaftGroup>, StudioError> {
        apply(self.behavior, mock::raft_groups).await
    }

    async fn shard_ranges(&self) -> Result<Vec<ShardRange>, StudioError> {
        apply(self.behavior, mock::shard_ranges).await
    }

    async fn users(&self) -> Result<Vec<UserRow>, StudioError> {
        apply(self.behavior, mock::users).await
    }

    async fn rls_policies(&self) -> Result<Vec<RlsPolicy>, StudioError> {
        apply(self.behavior, mock::rls_policies).await
    }

    async fn audit_entries(&self) -> Result<Vec<AuditEntry>, StudioError> {
        apply(self.behavior, mock::audit_entries).await
    }
}
