//! Mock fixtures, split per domain. The root re-exports each domain's public
//! surface.

mod admin;
mod cluster;
mod connections;
mod designer;
mod events;
mod explorer;
mod graph;
mod search;
mod spatial;
mod sync;
mod timeseries;
mod transfer;
mod vector;
mod workbench;

pub use admin::AdminStore;
pub use cluster::{ClusterStore, cluster_members, telemetry};
pub use connections::{
    connection_activity, connections, nav_badges, notifications, recent_items, saved_from_draft,
    session_info,
};
pub use designer::{DesignerStore, explorer_schema};
// Only the tests name this.
#[cfg(test)]
pub use designer::design_schema;
#[cfg(all(feature = "screenshots", target_os = "linux"))]
pub use events::seen_messages;
pub use events::{FeedStore, JobStore, ListenStore, TopicStore, ViewStore, preview_feed};
#[cfg(test)]
pub use events::{seeded_feeds, seeded_jobs};
pub use explorer::{
    GRAPH, Overlay, all_records, array_schema, array_slice, collection_access, collection_schema,
    collections, edge_page, observed_at, record_page, stored_record, transfer_preview,
};
pub use graph::{expand_node, find_paths, graph_bindings, rag_search, rag_setup, run_cypher};
#[cfg(all(feature = "screenshots", target_os = "linux"))]
pub use search::attached_query_vector;
pub use search::{
    SearchStore, search_analyze, search_indexes, search_rebuild_plan, search_run, search_setup,
};
pub use spatial::{
    raster_slice, spatial_analyze, spatial_feature, spatial_setup, spatial_validity,
    spatial_viewport,
};
pub use sync::SyncStore;
pub use timeseries::{
    BackfillError, DashError, ExploreError, LifecycleError, TimeseriesStore, backfill_preview,
    explore, time_sources,
};
pub use transfer::transfer_jobs;
pub use vector::{vector_index_status, vector_project, vector_record, vector_search, vector_setup};
pub use workbench::{WorkbenchStore, empty_result_set, query_plan, query_session, schema_tree};
// Only the screenshot scenes and the tests name these.
#[cfg(any(test, all(feature = "screenshots", target_os = "linux")))]
pub use workbench::CUSTOMER_SPEND_ID;
#[cfg(all(feature = "screenshots", target_os = "linux"))]
pub use workbench::{DATABASE as WORKBENCH_DATABASE, ORDERS_SQL, library as query_library};
