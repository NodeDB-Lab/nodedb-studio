//! The sidebar: brand, collapse control, and the grouped destinations.
//!
//! Capability-driven: a destination the active connection cannot serve does
//! not render. The active entry follows the current route, so every Cluster
//! tab lights "Cluster". Collapsed, the sidebar shows icons only, with each
//! label kept as the entry's tooltip and accessible name.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::resource_state;
use crate::components::icon::{Ico, Icon};
use crate::models::shell::NavBadges;
use crate::routes::{Destination, NAV_GROUPS, Route};
use crate::services::async_state::AsyncState;
use crate::services::backend::Backend;
use crate::state::connection::{ActiveConnection, Capabilities};
use crate::state::workspace::Workspace;

/// The badge text for `d`, or `None` for no badge.
///
/// Zero shows no badge. A load in flight or a failed read also shows none: a
/// badge is a hint, and the screen it points at reports its own load error.
fn badge_text(badges: &AsyncState<NavBadges>, d: Destination) -> Option<String> {
    let AsyncState::Loaded(b) = badges else {
        return None;
    };
    let n = match d {
        Destination::Query => b.query,
        Destination::Explorer
        | Destination::Designer
        | Destination::Events
        | Destination::Graph
        | Destination::Vector
        | Destination::Timeseries
        | Destination::Spatial
        | Destination::Search
        | Destination::Cluster
        | Destination::Sync
        | Destination::Administration
        | Destination::Console => 0,
    };
    (n > 0).then(|| n.to_string())
}

fn reachable(caps: Option<&Capabilities>, d: Destination) -> bool {
    match d.required_cap() {
        Some(cap) => caps.is_some_and(|c| c.has(cap)),
        None => true,
    }
}

#[component]
pub fn Sidebar() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let mut workspace = use_context::<Signal<Workspace>>();
    let route = use_route::<Route>();
    let badges = use_resource(move || {
        let backend = backend.clone();
        // Counts belong to the connection: reading its name re-runs this on
        // a switch.
        let _conn = conn();
        async move { backend.nav_badges().await }
    });
    let badges = resource_state(&badges);
    let collapsed = workspace.read().nav_collapsed();
    let caps = active.read().as_ref().map(|c| c.capabilities);
    let current = route.destination();
    let (toggle_icon, toggle_label) = if collapsed {
        (Icon::ChevronsRight, "Expand sidebar")
    } else {
        (Icon::ChevronsLeft, "Collapse sidebar")
    };

    rsx! {
        aside { class: "sidebar", "aria-label": "Navigation",
            div { class: "sidebar-brand",
                span { class: "brand-mark", Ico { icon: Icon::Database, size: 22 } }
                span { class: "brand-name", "NodeDB Studio" }
                button {
                    class: "icon-btn sidebar-toggle",
                    title: "{toggle_label}",
                    "aria-label": "{toggle_label}",
                    onclick: move |_| {
                        let now = workspace.peek().nav_collapsed();
                        workspace.write().set_nav_collapsed(!now);
                    },
                    Ico { icon: toggle_icon, size: 18 }
                }
            }
            nav { class: "sidebar-nav",
                for group in NAV_GROUPS.iter() {
                    div { key: "{group.label}", class: "nav-group",
                        div { class: "nav-group-label", "{group.label}" }
                        for d in group.items.iter().copied().filter(|d| reachable(caps.as_ref(), *d)) {
                            NavEntry {
                                key: "{d.label()}",
                                dest: d,
                                active: d == current,
                                badge: badge_text(&badges, d),
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn NavEntry(dest: Destination, active: bool, badge: Option<String>) -> Element {
    let nav = use_navigator();
    let label = dest.label();
    rsx! {
        button {
            class: if active { "nav-item active" } else { "nav-item" },
            title: "{label}",
            "aria-current": if active { "page" } else { "false" },
            onclick: move |_| { nav.push(dest.route()); },
            Ico { icon: dest.icon(), size: 20 }
            span { class: "nav-label", "{label}" }
            if let Some(b) = badge {
                span { class: "nav-badge", "aria-label": "{b} open", "{b}" }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::error::StudioError;

    #[test]
    fn query_count_renders_as_a_badge() {
        let b = AsyncState::Loaded(NavBadges { query: 3 });
        assert_eq!(badge_text(&b, Destination::Query), Some("3".to_string()));
        assert_eq!(badge_text(&b, Destination::Explorer), None);
    }

    #[test]
    fn zero_shows_no_badge() {
        let b = AsyncState::Loaded(NavBadges { query: 0 });
        assert_eq!(badge_text(&b, Destination::Query), None);
    }

    /// A failed or pending read must not invent a count.
    #[test]
    fn loading_and_error_show_no_badge() {
        assert_eq!(badge_text(&AsyncState::Loading, Destination::Query), None);
        let err = AsyncState::Error(StudioError::NotConnected);
        assert_eq!(badge_text(&err, Destination::Query), None);
    }

    #[test]
    fn gated_destinations_need_their_capability() {
        assert!(reachable(None, Destination::Explorer));
        assert!(!reachable(None, Destination::Cluster));
    }
}
