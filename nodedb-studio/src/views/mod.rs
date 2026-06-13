//! Top-level screens. The two-state machine in `crate::app` mounts exactly one
//! of these at a time: `ConnectionManager` when disconnected, `Studio` when
//! connected.

pub mod connection_manager;
pub mod studio_shell;
