//! Administration fixtures: principals and roles, row policies, the audit
//! trail, snapshots, config, and credentials, plus the store that applies
//! writes to them. Every applied change adds an event to the audit trail.

mod access;
mod audit;
mod backup;
mod config;
mod credential;
mod policy;
mod store;

pub use store::AdminStore;
