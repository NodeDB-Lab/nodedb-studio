//! What the Cluster page offers, grouped by what a user sees.

use serde::{Deserialize, Serialize};

use crate::routes::tabs::ClusterTab;
use crate::state::connection::Capabilities;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterCaps {
    /// Membership operations: leadership transfer, snapshot, replica moves.
    /// NodeDB 0.6.2 has no statement for any of them.
    pub operations: bool,
    /// Telemetry over time: the Monitoring charts. NodeDB 0.6.2 keeps no
    /// history.
    pub history: bool,
    /// The cluster event log: the Activity tab. NodeDB 0.6.2 has none.
    pub activity: bool,
}

impl ClusterCaps {
    /// The mock: everything.
    pub fn all() -> Self {
        Self {
            operations: true,
            history: true,
            activity: true,
        }
    }

    /// A NodeDB 0.6 server: point-in-time reads only.
    pub fn nodedb_0_6() -> Self {
        Self {
            operations: false,
            history: false,
            activity: false,
        }
    }
}

/// The Cluster tabs this connection offers, in display order.
pub fn cluster_tabs(caps: Option<&Capabilities>) -> Vec<ClusterTab> {
    ClusterTab::ALL
        .iter()
        .copied()
        .filter(|t| match t {
            ClusterTab::Nodes
            | ClusterTab::RaftGroups
            | ClusterTab::Placement
            | ClusterTab::Monitoring => true,
            ClusterTab::Activity => caps.is_none_or(|c| c.cluster_caps.activity),
        })
        .collect()
}

/// The tab to show for a route: itself when offered, else the first offered.
pub fn cluster_tab(tab: ClusterTab, caps: Option<&Capabilities>) -> ClusterTab {
    let offered = cluster_tabs(caps);
    if offered.contains(&tab) {
        tab
    } else {
        offered.first().copied().unwrap_or_default()
    }
}

/// Offer membership operations: leadership transfer, snapshot, rebalance.
pub fn offers_cluster_ops(caps: Option<&Capabilities>) -> bool {
    caps.is_none_or(|c| c.cluster_caps.operations)
}

/// Draw telemetry over time.
pub fn shows_cluster_history(caps: Option<&Capabilities>) -> bool {
    caps.is_none_or(|c| c.cluster_caps.history)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::gates::test_caps;

    fn nodedb() -> Capabilities {
        Capabilities {
            cluster_caps: ClusterCaps::nodedb_0_6(),
            ..test_caps()
        }
    }

    #[test]
    fn the_mock_offers_every_cluster_part() {
        let c = test_caps();
        assert_eq!(cluster_tabs(Some(&c)), ClusterTab::ALL);
        assert!(offers_cluster_ops(Some(&c)));
        assert!(shows_cluster_history(Some(&c)));
    }

    #[test]
    fn nodedb_0_6_shows_point_in_time_reads_only() {
        let c = nodedb();
        assert!(!cluster_tabs(Some(&c)).contains(&ClusterTab::Activity));
        assert_eq!(cluster_tabs(Some(&c)).len(), ClusterTab::ALL.len() - 1);
        assert!(!offers_cluster_ops(Some(&c)));
        assert!(!shows_cluster_history(Some(&c)));
    }

    #[test]
    fn a_hidden_tab_falls_back_to_the_first_offered() {
        let c = nodedb();
        assert_eq!(
            cluster_tab(ClusterTab::Activity, Some(&c)),
            ClusterTab::Nodes
        );
        assert_eq!(
            cluster_tab(ClusterTab::Placement, Some(&c)),
            ClusterTab::Placement
        );
        assert_eq!(
            cluster_tab(ClusterTab::Activity, Some(&test_caps())),
            ClusterTab::Activity
        );
    }

    #[test]
    fn no_connection_offers_everything() {
        assert_eq!(cluster_tabs(None), ClusterTab::ALL);
        assert!(offers_cluster_ops(None));
    }

    /// A profile saved before the flags existed came from the mock.
    #[test]
    fn a_profile_saved_before_the_flags_offers_them() {
        let old = r#"{"graph":true,"vector":true,"events":true,"timeseries":true,"spatial":true,"search":true,"sync":true,"cluster":true,"readonly":false}"#;
        let caps: Capabilities = sonic_rs::from_str(old).expect("loads");
        assert_eq!(caps.cluster_caps, ClusterCaps::all());
    }
}
