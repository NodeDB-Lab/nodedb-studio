//! Cluster: membership, Raft groups, placement, monitoring, and activity.
//! Scope is the whole Origin cluster, never the database in the header.

use dioxus::prelude::*;

use crate::components::tabs::{RouteTabs, tab_items};
use crate::routes::Route;
use crate::routes::tabs::ClusterTab;
use crate::views::admin::cluster::ClusterView;
use crate::views::admin::raft::RaftView;
use crate::views::admin::shards::ShardsView;
use crate::views::cluster::nodes::NodesTab;

#[component]
pub fn ClusterPage(tab: ClusterTab) -> Element {
    rsx! {
        RouteTabs {
            items: tab_items(ClusterTab::ALL, |t| t.label(), |tab| Route::Cluster { tab }),
            current: Route::Cluster { tab },
        }
        match tab {
            ClusterTab::Nodes => rsx! { NodesTab {} },
            ClusterTab::RaftGroups => rsx! { div { class: "legacy-page", RaftView {} } },
            ClusterTab::Placement => rsx! { div { class: "legacy-page", ShardsView {} } },
            ClusterTab::Monitoring => rsx! { div { class: "legacy-page", ClusterView {} } },
            ClusterTab::Activity => rsx! { div { class: "legacy-page" } },
        }
    }
}
