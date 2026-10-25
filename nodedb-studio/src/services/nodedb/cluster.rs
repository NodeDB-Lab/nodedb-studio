//! Cluster membership and replication reads.

use async_trait::async_trait;

use crate::models::cluster::{ClusterMembers, HostedGroup};
use crate::services::cluster_data::ClusterData;
use crate::services::error::StudioError;
use crate::services::nodedb::NodeDbConnectionService;

#[async_trait(?Send)]
impl ClusterData for NodeDbConnectionService {
    async fn cluster_members(&self) -> Result<ClusterMembers, StudioError> {
        Err(StudioError::NotConnected)
    }

    async fn hosted_groups(&self, _node_id: &str) -> Result<Vec<HostedGroup>, StudioError> {
        Err(StudioError::NotConnected)
    }
}
