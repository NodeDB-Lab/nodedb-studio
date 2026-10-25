//! Tab bars. `RouteTabs` switches pages through the router (Cluster →
//! Nodes, Raft groups, …). `TabBar` switches views inside one panel
//! (Data, Schema, Indexes) through a callback.

use dioxus::prelude::*;

use crate::routes::Route;

/// The page tabs of one tab set, e.g.
/// `tab_items(ClusterTab::ALL, |t| t.label(), |tab| Route::Cluster { tab })`.
pub fn tab_items<T: Copy>(
    all: &[T],
    label: impl Fn(T) -> &'static str,
    route: impl Fn(T) -> Route,
) -> Vec<(&'static str, Route)> {
    all.iter().map(|t| (label(*t), route(*t))).collect()
}

/// Page tabs under the header. The tab whose route equals `current` is active.
#[component]
pub fn RouteTabs(
    items: Vec<(&'static str, Route)>,
    current: Route,
    /// Controls at the right end of the strip, e.g. Open in Query.
    #[props(default = VNode::empty())]
    actions: Element,
) -> Element {
    let nav = use_navigator();
    rsx! {
        nav { class: "page-tabs", role: "tablist",
            for (label, route) in items {
                {
                    let selected = route == current;
                    rsx! {
                        button {
                            key: "{label}",
                            class: if selected { "tab active" } else { "tab" },
                            role: "tab",
                            "aria-selected": "{selected}",
                            onclick: move |_| { nav.push(route.clone()); },
                            "{label}"
                        }
                    }
                }
            }
            div { class: "page-tabs-actions", {actions} }
        }
    }
}

/// Tabs inside a panel. Labels are unique within one bar.
#[component]
pub fn TabBar(
    items: Vec<&'static str>,
    active: usize,
    on_select: EventHandler<usize>,
    #[props(default)] class: String,
) -> Element {
    rsx! {
        div { class: "tab-bar {class}", role: "tablist",
            for (i, label) in items.into_iter().enumerate() {
                button {
                    key: "{label}",
                    class: if i == active { "tab active" } else { "tab" },
                    role: "tab",
                    "aria-selected": "{i == active}",
                    onclick: move |_| on_select.call(i),
                    "{label}"
                }
            }
        }
    }
}
