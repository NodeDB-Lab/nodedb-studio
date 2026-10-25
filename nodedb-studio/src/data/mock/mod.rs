//! Mock fixtures, split per domain. Module root re-exports the same public
//! surface the single `mock.rs` used to expose, so callers are unchanged.
//!
//! Naming note: the mockup's legacy "arcadedb" labels, "local-arcade-dev"
//! name, "arcade-5" node, and per-version server tags are deliberately NOT
//! reproduced. NodeDB version numbers are not settled, so the
//! server stat is a neutral "dev" placeholder rather than an invented version.

mod admin;
mod cluster;
mod connections;
mod designer;
mod events;
mod explorer;
mod graph;
mod spatial;
#[cfg(test)]
mod test_support;
mod timeseries;
mod transfer;
mod vector;
mod viewers;
mod workbench;

pub use admin::{audit_entries, cluster_nodes, raft_groups, rls_policies, shard_ranges, users};
pub use cluster::{cluster_members, hosted_groups};
pub use connections::{
    connection_activity, connections, nav_badges, notifications, recent_items, saved_from_draft,
    session_info,
};
pub use designer::{DesignerStore, explorer_schema};
// Only the tests name this.
#[cfg(test)]
pub use designer::design_schema;
pub use events::{
    FeedStore, JobStore, ListenStore, TopicStore, ViewStore, preview_feed, seen_messages,
};
#[cfg(test)]
pub use events::{seeded_feeds, seeded_jobs};
pub use explorer::{
    GRAPH, Overlay, all_records, array_schema, array_slice, collection_access, collection_schema,
    collections, edge_page, observed_at, record_page, stored_record, transfer_preview,
};
pub use graph::{expand_node, find_paths, graph_bindings, rag_search, rag_setup, run_cypher};
pub use spatial::{
    raster_slice, spatial_analyze, spatial_feature, spatial_setup, spatial_validity,
    spatial_viewport,
};
pub use timeseries::{
    BackfillError, DashError, ExploreError, LifecycleError, TimeseriesStore, backfill_preview,
    explore, time_sources,
};
pub use transfer::transfer_jobs;
pub use vector::{vector_index_status, vector_project, vector_record, vector_search, vector_setup};
pub use viewers::{fts_hits, sync_peers};
pub use workbench::{WorkbenchStore, empty_result_set, query_plan, query_session, schema_tree};
// Only the screenshot scenes and the tests name these.
#[cfg(any(test, all(feature = "screenshots", target_os = "linux")))]
pub use workbench::{
    CUSTOMER_SPEND_ID, DATABASE as WORKBENCH_DATABASE, ORDERS_SQL, library as query_library,
};
