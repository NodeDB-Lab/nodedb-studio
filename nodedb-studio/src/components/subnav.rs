//! Admin sub-tabs. Cluster-only tabs carry `cap-hidden` (and are unclickable)
//! on single-node connections, mirroring the mockup's `data-cap="cluster"`.

use dioxus::prelude::*;

use crate::routes::Route;

/// (tab key, label, cluster-gated?)
const TABS: [(&str, &str, bool); 7] = [
    ("cluster", "Cluster", true),
    ("shards", "Shard map", true),
    ("nodes", "Per-node", true),
    ("raft", "Raft groups", true),
    ("rbac", "RBAC", false),
    ("rls", "RLS", false),
    ("audit", "Audit", false),
];

#[component]
pub fn AdminSubnav(active: String, cluster: bool) -> Element {
    let nav = use_navigator();
    rsx! {
        div { class: "subnav",
            for (key, label, gated) in TABS {
                {
                    let hidden = gated && !cluster;
                    let is_active = active == key;
                    let mut cls = String::from("subnav-item");
                    if is_active {
                        cls.push_str(" active");
                    }
                    if hidden {
                        cls.push_str(" cap-hidden");
                    }
                    let k = key.to_string();
                    rsx! {
                        div {
                            class: "{cls}",
                            onclick: move |_| {
                                if !hidden {
                                    nav.push(Route::Admin { tab: k.clone() });
                                }
                            },
                            "{label}"
                        }
                    }
                }
            }
        }
    }
}
