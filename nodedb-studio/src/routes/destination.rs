//! Sidebar destinations and their grouping.
//!
//! A destination is one sidebar entry. Several routes can share one (every
//! Cluster tab lights "Cluster"). The grouping is the accepted navigation:
//! WORKSPACE, TOOLS, OPERATIONS.

use crate::components::icon::Icon;
use crate::routes::Route;
use crate::state::connection::Capability;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Destination {
    Explorer,
    Query,
    Designer,
    Events,
    Graph,
    Vector,
    Timeseries,
    Spatial,
    Search,
    Cluster,
    Sync,
    Administration,
    Console,
}

/// One labelled sidebar section.
pub struct NavGroup {
    pub label: &'static str,
    pub items: &'static [Destination],
}

/// Sidebar order.
pub const NAV_GROUPS: [NavGroup; 3] = [
    NavGroup {
        label: "Workspace",
        items: &[
            Destination::Explorer,
            Destination::Query,
            Destination::Designer,
            Destination::Events,
        ],
    },
    NavGroup {
        label: "Tools",
        items: &[
            Destination::Graph,
            Destination::Vector,
            Destination::Timeseries,
            Destination::Spatial,
            Destination::Search,
        ],
    },
    NavGroup {
        label: "Operations",
        items: &[
            Destination::Cluster,
            Destination::Sync,
            Destination::Administration,
            Destination::Console,
        ],
    },
];

impl Destination {
    pub fn label(self) -> &'static str {
        match self {
            Destination::Explorer => "Explorer",
            Destination::Query => "Query",
            Destination::Designer => "Designer",
            Destination::Events => "Events & Jobs",
            Destination::Graph => "Graph Explorer",
            Destination::Vector => "Vector Space",
            Destination::Timeseries => "Timeseries",
            Destination::Spatial => "Spatial",
            Destination::Search => "Search",
            Destination::Cluster => "Cluster",
            Destination::Sync => "Sync",
            Destination::Administration => "Administration",
            Destination::Console => "Console",
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            Destination::Explorer => Icon::Folder,
            Destination::Query => Icon::FileCode,
            Destination::Designer => Icon::Workflow,
            Destination::Events => Icon::CalendarClock,
            Destination::Graph => Icon::GitFork,
            Destination::Vector => Icon::Grid3x3,
            Destination::Timeseries => Icon::ChartColumn,
            Destination::Spatial => Icon::MapPin,
            Destination::Search => Icon::Search,
            Destination::Cluster => Icon::Network,
            Destination::Sync => Icon::RefreshCw,
            Destination::Administration => Icon::Settings,
            Destination::Console => Icon::SquareTerminal,
        }
    }

    /// The route a sidebar click opens: the destination's first page.
    pub fn route(self) -> Route {
        match self {
            Destination::Explorer => Route::Explorer {},
            Destination::Query => Route::Query {},
            Destination::Designer => Route::Designer {},
            Destination::Events => Route::Events {
                tab: Default::default(),
            },
            Destination::Graph => Route::Graph {
                tab: Default::default(),
            },
            Destination::Vector => Route::Vector {
                tab: Default::default(),
            },
            Destination::Timeseries => Route::Timeseries {
                tab: Default::default(),
            },
            Destination::Spatial => Route::Spatial {
                tab: Default::default(),
            },
            Destination::Search => Route::Search {
                tab: Default::default(),
            },
            Destination::Cluster => Route::Cluster {
                tab: Default::default(),
            },
            Destination::Sync => Route::Sync {
                tab: Default::default(),
            },
            Destination::Administration => Route::Admin {
                tab: Default::default(),
            },
            Destination::Console => Route::Console {},
        }
    }

    /// The capability the active connection needs for this destination to
    /// appear. Exhaustive on purpose: a new destination must state whether it
    /// is gated, or a gated screen ships reachable on connections that cannot
    /// serve it.
    pub fn required_cap(self) -> Option<Capability> {
        match self {
            Destination::Events => Some(Capability::Events),
            Destination::Graph => Some(Capability::Graph),
            Destination::Vector => Some(Capability::Vector),
            Destination::Timeseries => Some(Capability::Timeseries),
            Destination::Spatial => Some(Capability::Spatial),
            Destination::Search => Some(Capability::Search),
            Destination::Cluster => Some(Capability::Cluster),
            Destination::Sync => Some(Capability::Sync),
            Destination::Explorer
            | Destination::Query
            | Destination::Designer
            | Destination::Administration
            | Destination::Console => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    /// Every destination appears in exactly one group.
    #[test]
    fn groups_list_each_destination_once() {
        let mut seen = HashSet::new();
        for group in &NAV_GROUPS {
            for d in group.items {
                assert!(seen.insert(*d), "{d:?} is listed twice");
            }
        }
        assert_eq!(seen.len(), 13);
    }

    /// A sidebar click lands on a route that lights the same entry.
    #[test]
    fn route_maps_back_to_its_destination() {
        for group in &NAV_GROUPS {
            for d in group.items {
                assert_eq!(d.route().destination(), *d);
            }
        }
    }
}
