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

const EVENTS_FEEDS: &[PanelSpec] = &[
    panel("feeds", "Change feeds", "Show the feed list"),
    panel("events", "Recent events", "Show the selected feed's events"),
    panel("details", "Feed details", "Show the selected feed"),
];

const EVENTS_VIEWS: &[PanelSpec] = &[
    panel("views", "Materialized views", "Show the saved views"),
    panel("result", "Result rows", "Show the selected view's rows"),
    panel(
        "definition",
        "Definition",
        "Show the definition and refresh policy",
    ),
    panel("history", "Refresh history", "Show the view's refreshes"),
];

const EVENTS_TOPICS: &[PanelSpec] = &[
    panel("topics", "Topics", "Show the durable topics"),
    panel("messages", "Messages", "Show the topic's messages"),
    panel("groups", "Consumer group", "Show committed offsets and lag"),
    panel("details", "Topic details", "Show the selected topic"),
];

const EVENTS_NOTIFICATIONS: &[PanelSpec] = &[
    panel(
        "listener",
        "Notification listener",
        "Show the listening session",
    ),
    panel("send", "Send notification", "Show the send form"),
];

const EVENTS_JOBS: &[PanelSpec] = &[
    panel("jobs", "Jobs", "Show the scheduled jobs"),
    panel("details", "Job details", "Show the selected job"),
    panel("runs", "Job runs", "Show the job's runs"),
    panel("log", "Run log", "Show the selected run's log"),
];

const GRAPH_CYPHER: &[PanelSpec] = &[
    panel("query", "Query", "Show the Cypher editor"),
    panel("selection", "Selection", "Show the selected node or edge"),
];

const GRAPH_BUILDER: &[PanelSpec] = &[
    panel("build", "Build query", "Show the pattern form"),
    panel("graph", "Graph view", "Show the result graph"),
    panel("details", "Node details", "Show the selected node or edge"),
];

const GRAPH_RAG: &[PanelSpec] = &[
    panel("query", "Query", "Show the question and retrieval source"),
    panel(
        "evidence",
        "Retrieved evidence",
        "Show the retrieved records",
    ),
    panel(
        "context",
        "Graph neighbor context",
        "Show the graph around the evidence",
    ),
    panel(
        "selected",
        "Selected evidence",
        "Show the chosen record's citation",
    ),
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
            Surface::Events(EventsTab::ChangeFeeds) => EVENTS_FEEDS,
            Surface::Events(EventsTab::MaterializedViews) => EVENTS_VIEWS,
            Surface::Events(EventsTab::Topics) => EVENTS_TOPICS,
            Surface::Events(EventsTab::Notifications) => EVENTS_NOTIFICATIONS,
            Surface::Events(EventsTab::ScheduledJobs) => EVENTS_JOBS,
            Surface::Graph(GraphTab::Cypher) => GRAPH_CYPHER,
            Surface::Graph(GraphTab::Builder) => GRAPH_BUILDER,
            Surface::Graph(GraphTab::GraphRag) => GRAPH_RAG,
            Surface::Vector(VectorTab::Search | VectorTab::Projection | VectorTab::Index)
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
