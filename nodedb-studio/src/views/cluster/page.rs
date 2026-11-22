//! Cluster: membership, Raft groups, placement, monitoring, and activity.
//! Scope is the whole Origin cluster, never the database in the header.

use dioxus::prelude::*;

use crate::components::tabs::{RouteTabs, tab_items};
use crate::routes::Route;
use crate::routes::tabs::ClusterTab;
use crate::state::caps::cluster::{cluster_tab, cluster_tabs};
use crate::state::gates::use_caps;
use crate::views::cluster::activity::ActivityTab;
use crate::views::cluster::monitor::MonitoringTab;
use crate::views::cluster::nodes::NodesTab;
use crate::views::cluster::placement::PlacementTab;
use crate::views::cluster::raft::RaftTab;

#[component]
pub fn ClusterPage(tab: ClusterTab) -> Element {
    let caps = use_caps();
    // A route to a tab this server cannot serve shows the first offered one.
    let tab = cluster_tab(tab, caps.as_ref());
    rsx! {
        RouteTabs {
            items: tab_items(&cluster_tabs(caps.as_ref()), |t| t.label(), |tab| Route::Cluster { tab }),
            current: Route::Cluster { tab },
        }
        match tab {
            ClusterTab::Nodes => rsx! { NodesTab {} },
            ClusterTab::RaftGroups => rsx! { RaftTab {} },
            ClusterTab::Placement => rsx! { PlacementTab {} },
            ClusterTab::Monitoring => rsx! { MonitoringTab {} },
            ClusterTab::Activity => rsx! { ActivityTab {} },
        }
    }
}
