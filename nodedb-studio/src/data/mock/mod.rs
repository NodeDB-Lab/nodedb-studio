//! Mock fixtures, split per domain. Module root re-exports the same public
//! surface the single `mock.rs` used to expose, so callers are unchanged.
//!
//! Naming note: the mockup's legacy "arcadedb" labels, "local-arcade-dev"
//! name, "arcade-5" node, and per-version server tags are deliberately NOT
//! reproduced. NodeDB version numbers are undecided (CLAUDE.md §2), so the
//! server stat is a neutral "dev" placeholder rather than an invented version.

mod admin;
mod cdc;
mod connections;
mod docs;
mod explorer;
mod notify;
mod streams;
mod viewers;
mod workbench;

pub use admin::{audit_entries, cluster_nodes, raft_groups, rls_policies, shard_ranges, users};
pub use cdc::{ChangeOp, cdc_events};
pub use connections::{connections, explorer_collections, notifications};
pub use explorer::{collection_groups, record_detail, records};
pub use notify::{notify_channels, notify_messages};
pub use streams::{
    materialized_views, notify_channel_rows, notify_message_rows, scheduled_jobs, topics,
};
pub use viewers::{fts_hits, series, spatial_features, sub_graph, sync_peers, vector_points};
pub use workbench::{query_plan, result_set, schema_tree};
