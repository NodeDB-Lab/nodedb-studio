//! Service traits at the backend seam. The mock impl is the only one today;
//! a NodeDB-client-backed impl plugs in here later.

pub mod async_state;
pub mod backend;
pub mod connection_service;
pub mod decode;
pub mod error;
pub mod mock_behavior;
pub mod nodedb_service;
pub mod streams_data;
