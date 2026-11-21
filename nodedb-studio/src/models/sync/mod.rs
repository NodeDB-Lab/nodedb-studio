//! Sync devices: the Lite instances an Origin cluster knows, with what each
//! one reported and what the Origin measured.
//!
//! A connected transport says nothing about how far the data got. Each stage
//! (received, applied, acknowledged) is its own fact with its own time, and a
//! stage nobody reported stays `None` and shows as Unknown. A pending count
//! belongs to the device that reported it.

pub mod activity;
mod base;
pub mod enroll;
pub mod reject;
pub mod sub;
pub use base::*;
