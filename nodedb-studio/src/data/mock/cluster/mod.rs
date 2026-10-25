//! Cluster fixtures: members, Raft groups, placement, telemetry, and
//! activity, plus the store that applies writes to them.

pub mod activity;
pub mod members;
pub mod placement;
pub mod raft;
pub mod store;
pub mod telemetry;

pub use members::cluster_members;
pub use store::ClusterStore;
pub use telemetry::telemetry;
