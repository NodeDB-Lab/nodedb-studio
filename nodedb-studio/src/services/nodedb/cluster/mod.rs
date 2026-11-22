//! Cluster reads over the `SHOW` statements of the node Studio connected
//! to. That node is the observer: every role, leader, term, node state and
//! circuit state is its opinion, and every read says so.
//!
//! Wired: members (`SHOW CLUSTER`, `SHOW NODES`, `SHOW PEER HEALTH`), the
//! observer's own groups (`SHOW RAFT GROUPS`, `SHOW RAFT GROUP <id>`),
//! placement (`SHOW RANGES`, `SHOW MIGRATIONS`), and the observer's engine
//! budgets (`SHOW MEMORY`). Operations, history and the activity log have no
//! source on NodeDB 0.6.2 and return `Unsupported`. Studio sends no
//! statement that changes the cluster.

pub mod assemble;
mod base;
pub mod decode;
