//! Admin. The `tab` route segment selects the sub-tab. The four cluster tabs
//! (cluster/shards/nodes/raft) hide on single-node connections; if the active
//! tab is cluster-gated on such a connection, it falls back to RBAC.

use dioxus::prelude::*;

use crate::components::subnav::AdminSubnav;
use crate::state::connection::ActiveConnection;
use crate::views::admin::audit::AuditView;
use crate::views::admin::cluster::ClusterView;
use crate::views::admin::nodes::NodesView;
use crate::views::admin::raft::RaftView;
use crate::views::admin::rbac::RbacView;
use crate::views::admin::rls::RlsView;
use crate::views::admin::shards::ShardsView;

fn is_cluster_tab(tab: &str) -> bool {
    matches!(tab, "cluster" | "shards" | "nodes" | "raft")
}

#[component]
pub fn Admin(tab: String) -> Element {
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let cluster = active
        .read()
        .as_ref()
        .map(|c| c.capabilities.cluster)
        .unwrap_or(false);

    // A cluster-gated tab on a single-node connection falls back to RBAC.
    let effective = if is_cluster_tab(&tab) && !cluster {
        "rbac".to_string()
    } else {
        tab.clone()
    };

    rsx! {
        div { class: "view active",
            AdminSubnav { active: effective.clone(), cluster }
            match effective.as_str() {
                "cluster" => rsx! { ClusterView {} },
                "shards" => rsx! { ShardsView {} },
                "nodes" => rsx! { NodesView {} },
                "raft" => rsx! { RaftView {} },
                "rls" => rsx! { RlsView {} },
                "audit" => rsx! { AuditView {} },
                _ => rsx! { RbacView {} },
            }
        }
    }
}
