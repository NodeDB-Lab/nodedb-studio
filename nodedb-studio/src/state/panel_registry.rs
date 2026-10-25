//! The closable panels of every page, in View menu order.
//!
//! A page renders each listed panel through `components::panel::Panel` with
//! the same id. The View menu adds the global "Navigation" entry above
//! these, so it is never listed here.

use crate::routes::tabs::{
    AdminTab, ClusterTab, EventsTab, GraphTab, SearchTab, SpatialTab, SyncTab, TimeseriesTab,
    VectorTab,
};
use crate::state::workspace::{PanelSpec, Surface};

const fn panel(id: &'static str, label: &'static str, hint: &'static str) -> PanelSpec {
    PanelSpec { id, label, hint }
}

const CONNECTIONS: &[PanelSpec] = &[
    panel("saved", "Saved connections", "Show the connection list"),
    panel(
        "details",
        "Connection details",
        "Show the selected connection",
    ),
    panel("activity", "Recent activity", "Show the connection log"),
];

const EXPLORER: &[PanelSpec] = &[
    panel("catalog", "Catalog", "Show collection list"),
    panel("records", "Records", "Show data view"),
    panel("details", "Record details", "Show inspector panel"),
];

const DATA_TRANSFER: &[PanelSpec] = &[
    panel("transfer", "Transfer", "Show import and export"),
    panel("jobs", "Background jobs", "Show import and export jobs"),
];

const QUERY: &[PanelSpec] = &[
    panel("library", "Saved queries", "Show query library"),
    panel("schema", "Schema", "Show tables and fields"),
    panel("results", "Results", "Show results, plan, and messages"),
];

const CONSOLE: &[PanelSpec] = &[
    panel("console", "Console", "Show the session and transcript"),
    panel("history", "Command history", "Show earlier commands"),
];

const CLUSTER_NODES: &[PanelSpec] = &[
    panel("nodes", "Nodes", "Show configured members"),
    panel(
        "groups",
        "Replication groups",
        "Show groups hosted on the node",
    ),
    panel("details", "Node details", "Show the selected node"),
];

const DESIGNER: &[PanelSpec] = &[panel(
    "properties",
    "Properties",
    "Show the selected object's definition",
)];

impl Surface {
    pub fn panels(self) -> &'static [PanelSpec] {
        match self {
            Surface::Connections => CONNECTIONS,
            Surface::Explorer => EXPLORER,
            Surface::DataTransfer => DATA_TRANSFER,
            Surface::Query => QUERY,
            Surface::Console => CONSOLE,
            Surface::Designer => DESIGNER,
            Surface::Cluster(ClusterTab::Nodes) => CLUSTER_NODES,
            Surface::Events(
                EventsTab::ChangeFeeds
                | EventsTab::MaterializedViews
                | EventsTab::Topics
                | EventsTab::Notifications
                | EventsTab::ScheduledJobs,
            )
            | Surface::Graph(GraphTab::Cypher | GraphTab::Builder | GraphTab::GraphRag)
            | Surface::Vector(VectorTab::Search | VectorTab::Projection | VectorTab::Index)
            | Surface::Timeseries(
                TimeseriesTab::Explore | TimeseriesTab::Dashboards | TimeseriesTab::Lifecycle,
            )
            | Surface::Spatial(SpatialTab::Map | SpatialTab::Analyze | SpatialTab::Raster)
            | Surface::Search(SearchTab::Results | SearchTab::Analyze | SearchTab::Indexes)
            | Surface::Cluster(
                ClusterTab::RaftGroups
                | ClusterTab::Placement
                | ClusterTab::Monitoring
                | ClusterTab::Activity,
            )
            | Surface::Sync(
                SyncTab::Devices
                | SyncTab::Subscriptions
                | SyncTab::RejectedWrites
                | SyncTab::Activity,
            )
            | Surface::Admin(
                AdminTab::Access
                | AdminTab::RowPolicies
                | AdminTab::Audit
                | AdminTab::Backups
                | AdminTab::Config
                | AdminTab::Credentials,
            ) => &[],
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn every_surface() -> Vec<Surface> {
        let mut all = vec![
            Surface::Connections,
            Surface::Explorer,
            Surface::DataTransfer,
            Surface::Query,
            Surface::Designer,
            Surface::Console,
        ];
        all.extend(EventsTab::ALL.iter().map(|t| Surface::Events(*t)));
        all.extend(GraphTab::ALL.iter().map(|t| Surface::Graph(*t)));
        all.extend(VectorTab::ALL.iter().map(|t| Surface::Vector(*t)));
        all.extend(TimeseriesTab::ALL.iter().map(|t| Surface::Timeseries(*t)));
        all.extend(SpatialTab::ALL.iter().map(|t| Surface::Spatial(*t)));
        all.extend(SearchTab::ALL.iter().map(|t| Surface::Search(*t)));
        all.extend(ClusterTab::ALL.iter().map(|t| Surface::Cluster(*t)));
        all.extend(SyncTab::ALL.iter().map(|t| Surface::Sync(*t)));
        all.extend(AdminTab::ALL.iter().map(|t| Surface::Admin(*t)));
        all
    }

    /// Two panels with one id would hide and show together.
    #[test]
    fn panel_ids_are_unique_per_page() {
        for surface in every_surface() {
            let mut seen = HashSet::new();
            for p in surface.panels() {
                assert!(seen.insert(p.id), "{surface:?} lists `{}` twice", p.id);
                assert_ne!(
                    p.id, "navigation",
                    "Navigation is the View menu's own entry"
                );
            }
        }
    }

    #[test]
    fn surface_keys_are_unique() {
        let mut seen = HashSet::new();
        for surface in every_surface() {
            assert!(seen.insert(surface.key()), "{surface:?} key is not unique");
        }
    }
}
