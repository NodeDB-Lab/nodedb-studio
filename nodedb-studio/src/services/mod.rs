//! Service traits at the backend seam. The mock impl is the only one today;
//! a NodeDB-client-backed impl plugs in here later.

pub mod admin_data;
pub mod async_state;
pub mod backend;
pub mod cluster_data;
pub mod connection_service;
pub mod decode;
pub mod designer_data;
pub mod error;
pub mod explorer_data;
pub mod mock;
pub mod mock_behavior;
pub mod nodedb;
pub mod streams_data;
pub mod transfer_data;
pub mod viewers_data;
pub mod workbench_data;

pub use mock::MockConnectionService;
