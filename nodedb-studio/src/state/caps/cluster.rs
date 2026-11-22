//! What the Cluster page offers, grouped by what a user sees.

use serde::{Deserialize, Serialize};

use crate::routes::tabs::ClusterTab;
use crate::state::connection::Capabilities;
use crate::state::workspace::Surface;

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

/// Whether the View menu lists panel `id` of `surface`. A panel a Cluster
/// flag hides renders nothing, so its toggle must not show. Every panel of
/// another page is listed.
pub fn offers_panel(surface: Surface, id: &str, caps: Option<&Capabilities>) -> bool {
    match surface {
        Surface::Cluster(ClusterTab::Placement) if id == "rebalance" => offers_cluster_ops(caps),
        Surface::Cluster(ClusterTab::Monitoring) if id != "nodes" => shows_cluster_history(caps),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use dioxus::prelude::*;

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

    /// The View menu of a hidden part must not list its panels: toggling
    /// one renders nothing.
    #[test]
    fn the_view_menu_lists_no_panel_a_flag_hides() {
        use crate::components::view_menu::ViewMenuFor;
        use crate::state::connection::ActiveConnection;
        use crate::state::workspace::Workspace;

        #[component]
        fn Menu(surface: Surface) -> Element {
            use_context_provider(|| Signal::new(Workspace::default()));
            use_context_provider(|| {
                Signal::new(Some(ActiveConnection {
                    name: "n".into(),
                    sub: String::new(),
                    display_name: String::new(),
                    user: "nodedb".into(),
                    role: String::new(),
                    capabilities: nodedb(),
                    current_database: "default".into(),
                }))
            });
            rsx! { ViewMenuFor { surface, navigation: false } }
        }
        let render = |surface| {
            let mut dom = VirtualDom::new_with_props(Menu, MenuProps { surface });
            dom.rebuild_in_place();
            dioxus_ssr::render(&dom)
        };
        let place = render(Surface::Cluster(ClusterTab::Placement));
        assert!(place.contains("vShard details"), "{place}");
        assert!(!place.contains("Rebalance preview"), "{place}");
        let monitor = render(Surface::Cluster(ClusterTab::Monitoring));
        assert!(monitor.contains("Nodes"), "{monitor}");
        for chart in ["Queries", "p99 latency", "CPU usage", "Memory usage"] {
            assert!(!monitor.contains(chart), "{chart}: {monitor}");
        }
    }

    /// A profile saved before the flags existed came from the mock.
    #[test]
    fn a_profile_saved_before_the_flags_offers_them() {
        let old = r#"{"graph":true,"vector":true,"events":true,"timeseries":true,"spatial":true,"search":true,"sync":true,"cluster":true,"readonly":false}"#;
        let caps: Capabilities = sonic_rs::from_str(old).expect("loads");
        assert_eq!(caps.cluster_caps, ClusterCaps::all());
    }
}
