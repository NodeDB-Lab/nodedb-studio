//! Sync fixtures: devices, subscriptions, rejected writes, and activity,
//! plus the store that applies writes to them.

mod activity;
mod devices;
mod rejects;
mod store;
mod store_enroll;
mod store_reject;
mod subs;

pub use store::SyncStore;
