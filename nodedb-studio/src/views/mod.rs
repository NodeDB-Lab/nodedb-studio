//! Top-level screens.
//!
//! The two-state machine in `crate::app` mounts `ConnectionManager` when
//! disconnected and `Studio` when connected. Inside `Studio`, the router mounts
//! exactly one of the studio views below into the content `Outlet`.

pub mod connection_manager;
pub mod studio_shell;

// Studio views (routed). One module per rail destination; each renders the
// faithful per-view content ported from the mockup.
pub mod admin;
pub mod console;
pub mod designer;
pub mod explorer;
pub mod fts_inspector;
pub mod graph_explorer;
pub mod query;
pub mod spatial_view;
pub mod streams;
pub mod sync;
pub mod timeseries_dashboard;
pub mod vector_space;
