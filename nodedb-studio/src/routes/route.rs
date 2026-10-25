//! The studio route table.
//!
//! Routing exists only in the connected state: `App` mounts the `Router`
//! inside `Studio`, never in the Connection Manager. Every route shares
//! `StudioLayout`, which renders the persistent shell around the `Outlet`.

use dioxus::prelude::*;

use crate::routes::destination::Destination;
use crate::routes::layout::StudioLayout;
use crate::routes::tabs::{
    AdminTab, ClusterTab, EventsTab, GraphTab, SearchTab, SpatialTab, SyncTab, TimeseriesTab,
    VectorTab,
};
use crate::state::workspace::Surface;
use crate::views::cluster::ClusterPage;
use crate::views::explorer::{DataTransferPage, ExplorerPage};
use crate::views::transitional::{
    AdminPage, ConsolePage, DesignerPage, EventsPage, GraphPage, QueryPage, SearchPage,
    SpatialPage, SyncPage, TimeseriesPage, VectorPage,
};

#[derive(Routable, Clone, PartialEq, Debug)]
pub enum Route {
    #[layout(StudioLayout)]
    #[route("/", ExplorerPage)]
    Explorer {},
    #[route("/explorer/transfer", DataTransferPage)]
    DataTransfer {},
    #[route("/query", QueryPage)]
    Query {},
    #[route("/designer", DesignerPage)]
    Designer {},
    #[route("/events/:tab", EventsPage)]
    Events { tab: EventsTab },
    #[route("/graph/:tab", GraphPage)]
    Graph { tab: GraphTab },
    #[route("/vector/:tab", VectorPage)]
    Vector { tab: VectorTab },
    #[route("/timeseries/:tab", TimeseriesPage)]
    Timeseries { tab: TimeseriesTab },
    #[route("/spatial/:tab", SpatialPage)]
    Spatial { tab: SpatialTab },
    #[route("/search/:tab", SearchPage)]
    Search { tab: SearchTab },
    #[route("/cluster/:tab", ClusterPage)]
    Cluster { tab: ClusterTab },
    #[route("/sync/:tab", SyncPage)]
    Sync { tab: SyncTab },
    #[route("/admin/:tab", AdminPage)]
    Admin { tab: AdminTab },
    #[route("/console", ConsolePage)]
    Console {},
}

impl Route {
    /// The sidebar entry this route lights.
    pub fn destination(&self) -> Destination {
        match self {
            Route::Explorer {} | Route::DataTransfer {} => Destination::Explorer,
            Route::Query {} => Destination::Query,
            Route::Designer {} => Destination::Designer,
            Route::Events { .. } => Destination::Events,
            Route::Graph { .. } => Destination::Graph,
            Route::Vector { .. } => Destination::Vector,
            Route::Timeseries { .. } => Destination::Timeseries,
            Route::Spatial { .. } => Destination::Spatial,
            Route::Search { .. } => Destination::Search,
            Route::Cluster { .. } => Destination::Cluster,
            Route::Sync { .. } => Destination::Sync,
            Route::Admin { .. } => Destination::Administration,
            Route::Console {} => Destination::Console,
        }
    }

    /// The page whose panels the View menu lists.
    pub fn surface(&self) -> Surface {
        match self {
            Route::Explorer {} => Surface::Explorer,
            Route::DataTransfer {} => Surface::DataTransfer,
            Route::Query {} => Surface::Query,
            Route::Designer {} => Surface::Designer,
            Route::Events { tab } => Surface::Events(*tab),
            Route::Graph { tab } => Surface::Graph(*tab),
            Route::Vector { tab } => Surface::Vector(*tab),
            Route::Timeseries { tab } => Surface::Timeseries(*tab),
            Route::Spatial { tab } => Surface::Spatial(*tab),
            Route::Search { tab } => Surface::Search(*tab),
            Route::Cluster { tab } => Surface::Cluster(*tab),
            Route::Sync { tab } => Surface::Sync(*tab),
            Route::Admin { tab } => Surface::Admin(*tab),
            Route::Console {} => Surface::Console,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_routes_parse_from_their_url() {
        let route = "/cluster/raft-groups".parse::<Route>().ok();
        assert!(matches!(
            route,
            Some(Route::Cluster {
                tab: ClusterTab::RaftGroups
            })
        ));
    }

    #[test]
    fn transfer_page_lights_explorer() {
        assert_eq!(Route::DataTransfer {}.destination(), Destination::Explorer);
    }
}
