//! Specialized-viewer models: sync.

use serde::{Deserialize, Serialize};

/// One peer in the sync/replication topology.
#[allow(dead_code)] // SEAM-UNWIRED
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncPeer {
    pub id: String,
    pub name: String,
    pub state: String,
    pub lag: String,
}
