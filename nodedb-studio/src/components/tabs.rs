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
        PageTabStrip {
            items,
            current,
            actions,
            on_select: move |route: Route| { nav.push(route); },
        }
    }
}

/// The strip `RouteTabs` draws: separate so it renders without a router.
#[component]
fn PageTabStrip(
    items: Vec<(&'static str, Route)>,
    current: Route,
    actions: Element,
    on_select: EventHandler<Route>,
) -> Element {
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
                            onclick: move |_| on_select.call(route.clone()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routes::tabs::AdminTab;
    use crate::state::connection::Capabilities;
    use crate::state::gates::admin_tabs;

    fn strip(caps: Option<Capabilities>) -> String {
        #[derive(Props, Clone, PartialEq)]
        struct P {
            caps: Option<Capabilities>,
        }
        let mut dom = VirtualDom::new_with_props(
            |p: P| {
                let items = tab_items(
                    &admin_tabs(p.caps.as_ref()),
                    |t| t.label(),
                    |tab| Route::Admin { tab },
                );
                rsx! {
                    PageTabStrip {
                        items,
                        current: Route::Admin { tab: AdminTab::Access },
                        actions: VNode::empty(),
                        on_select: |_| {},
                    }
                }
            },
            P { caps },
        );
        dom.rebuild_in_place();
        dioxus_ssr::render(&dom)
    }

    #[test]
    fn admin_strip_hides_tabs_the_server_cannot_serve() {
        let mut caps = crate::state::gates::test_caps();
        assert!(strip(Some(caps)).contains("Backups"));
        caps.backups = false;
        caps.config = false;
        caps.credentials = false;
        let html = strip(Some(caps));
        for gone in ["Backups", "Config", "Credentials"] {
            assert!(!html.contains(gone), "{gone} still shown: {html}");
        }
        assert!(html.contains("Access") && html.contains("Audit"), "{html}");
    }
}
