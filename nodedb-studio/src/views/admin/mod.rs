//! Admin. The `tab` selects the sub-tab (cluster/shards/nodes/raft/rbac/rls/
//! audit); cluster-only tabs hide on single-node connections in Phase 4.
use dioxus::prelude::*;
use crate::views::ViewPlaceholder;

#[component]
pub fn Admin(tab: String) -> Element {
    rsx! { ViewPlaceholder { title: "Admin", note: "Admin sub-tab \"{tab}\" placeholder — Phase 4." } }
}
