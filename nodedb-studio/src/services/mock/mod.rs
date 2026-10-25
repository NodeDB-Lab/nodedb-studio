//! The mock backend, one file per domain trait.

pub mod admin;
pub mod cluster;
pub mod connection;
pub mod designer;
pub mod explorer;
pub mod feeds;
pub mod jobs;
pub mod listen;
pub mod matviews;
pub mod service;
pub mod topics;
pub mod transfer;
pub mod viewers;
pub mod workbench;

pub use service::MockConnectionService;
