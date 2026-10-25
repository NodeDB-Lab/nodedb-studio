//! Cluster: membership, Raft groups, placement, monitoring, and activity.
//! Scope is the whole Origin cluster, never the database in the header.

use dioxus::prelude::*;

use crate::components::tabs::{RouteTabs, tab_items};
use crate::routes::Route;
use crate::routes::tabs::ClusterTab;
use crate::views::cluster::activity::ActivityTab;
use crate::views::cluster::monitor::MonitoringTab;
use crate::views::cluster::nodes::NodesTab;
use crate::views::cluster::placement::PlacementTab;
use crate::views::cluster::raft::RaftTab;

#[component]
pub fn ClusterPage(tab: ClusterTab) -> Element {
    rsx! {
        RouteTabs {
            items: tab_items(ClusterTab::ALL, |t| t.label(), |tab| Route::Cluster { tab }),
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
