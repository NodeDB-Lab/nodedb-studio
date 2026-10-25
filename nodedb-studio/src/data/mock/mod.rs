//! Mock fixtures, split per domain. Module root re-exports the same public
//! surface the single `mock.rs` used to expose, so callers are unchanged.
//!
//! Naming note: the mockup's legacy "arcadedb" labels, "local-arcade-dev"
//! name, "arcade-5" node, and per-version server tags are deliberately NOT
//! reproduced. NodeDB version numbers are not settled, so the
//! server stat is a neutral "dev" placeholder rather than an invented version.

mod admin;
mod cdc;
mod cluster;
mod connections;
mod docs;
mod explorer;
mod streams;
#[cfg(test)]
mod test_support;
mod transfer;
mod viewers;
mod workbench;

pub use admin::{audit_entries, cluster_nodes, raft_groups, rls_policies, shard_ranges, users};
pub use cdc::{ChangeOp, cdc_events};
pub use cluster::{cluster_members, hosted_groups};
pub use connections::{
    connection_activity, connections, nav_badges, notifications, recent_items, saved_from_draft,
    session_info,
};
pub use explorer::{
    GRAPH, Overlay, array_schema, array_slice, collection_access, collection_schema, collections,
    edge_page, observed_at, record_page, stored_record, transfer_preview,
};
pub use streams::{
    materialized_views, notify_channel_rows, notify_message_rows, scheduled_jobs, topics,
};
pub use transfer::transfer_jobs;
pub use viewers::{
    empty_sub_graph, fts_hits, series, spatial_features, sub_graph, sync_peers, vector_points,
};
pub use workbench::{empty_result_set, query_plan, result_set, schema_tree};
