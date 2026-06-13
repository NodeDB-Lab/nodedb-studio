//! Admin views.

pub mod audit;
pub mod cluster;
pub mod nodes;
pub mod raft;
pub mod rbac;
pub mod rls;
pub mod shards;
mod view;

pub use view::Admin;
