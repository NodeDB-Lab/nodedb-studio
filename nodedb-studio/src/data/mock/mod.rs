//! Mock fixtures, split per domain. Module root re-exports the same public
//! surface the single `mock.rs` used to expose, so callers are unchanged.
//!
//! Naming note: the mockup's legacy "arcadedb" labels, "local-arcade-dev"
//! name, "arcade-5" node, and per-version server tags are deliberately NOT
//! reproduced. NodeDB version numbers are undecided (CLAUDE.md §2), so the
//! server stat is a neutral "dev" placeholder rather than an invented version.

mod cdc;
mod connections;
mod docs;
mod notify;

pub use cdc::{ChangeOp, cdc_events};
pub use connections::{connections, explorer_collections, notifications};
pub use notify::{notify_channels, notify_messages};
