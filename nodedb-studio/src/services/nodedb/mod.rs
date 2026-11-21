//! The real-client backend, one file per domain trait.

pub mod admin;
pub mod admin_access;
pub mod admin_audit;
pub mod admin_policy;
pub mod cluster;
pub mod connection;
pub mod designer;
pub mod explorer;
pub mod explorer_access;
pub mod explorer_arrays;
pub mod explorer_graph;
pub mod explorer_records;
pub mod explorer_sql;
pub mod explorer_write;
pub mod feeds;
pub mod graph;
pub mod jobs;
pub mod listen;
pub mod matviews;
pub mod search;
pub mod service;
pub mod spatial;
pub mod sync;
pub mod timeseries;
pub mod topics;
pub mod transfer;
pub mod vector;
pub mod workbench;
pub mod workbench_conn;
pub mod workbench_sql;
pub mod workbench_tx;

pub use service::NodeDbConnectionService;
