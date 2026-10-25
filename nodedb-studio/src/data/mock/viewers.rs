//! Specialized-viewer fixtures: sync.

use crate::models::viewers::SyncPeer;

/// Sync/replication peers.
#[allow(dead_code)] // SEAM-UNWIRED
pub fn sync_peers() -> Vec<SyncPeer> {
    vec![
        SyncPeer {
            id: "peer-1".into(),
            name: "eu-west".into(),
            state: "synced".into(),
            lag: "0ms".into(),
        },
        SyncPeer {
            id: "peer-2".into(),
            name: "ap-south".into(),
            state: "lagging".into(),
            lag: "820ms".into(),
        },
    ]
}
