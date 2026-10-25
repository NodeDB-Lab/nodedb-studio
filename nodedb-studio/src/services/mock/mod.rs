//! The mock backend, one file per domain trait.

pub mod admin;
pub mod cluster;
pub mod connection;
pub mod explorer;
pub mod service;
pub mod streams;
pub mod transfer;
pub mod viewers;
pub mod workbench;

pub use service::MockConnectionService;
