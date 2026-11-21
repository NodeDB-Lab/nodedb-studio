//! Cluster membership and replication, as an Origin cluster reports them.
//!
//! NodeDB runs independent metadata, data, and sequencer Raft groups.
//! Leadership, term, log positions, and quorum belong to a group, never to
//! the cluster: no type here names a cluster-wide leader. Reachability (a
//! node answers) and replication readiness (a group has quorum) are separate
//! facts with separate types.

pub mod activity;
mod base;
pub mod chart;
pub mod monitor;
pub mod op;
pub mod place;
pub mod raft;
pub use base::*;
