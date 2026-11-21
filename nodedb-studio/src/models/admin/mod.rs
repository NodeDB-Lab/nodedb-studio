//! What every Administration change shares: the review a person reads
//! before anything applies, and the one enum that names every change.
//!
//! A change is previewed first. The preview carries the revision of the
//! objects it read, and apply names that revision. The server refuses an
//! apply when the objects changed in between, so a stale review never
//! overwrites another administrator's work.

pub mod access;
pub mod access_change;
pub mod audit;
pub mod backup;
mod base;
pub mod config;
pub mod credential;
pub mod grant;
pub mod password;
pub mod policy;
pub use base::*;
