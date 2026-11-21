//! Change feeds: a database object that captures the changes of one
//! collection and delivers them to consumers.
//!
//! A feed's state is its server delivery. An event's delivery status belongs
//! to one consumer, and several consumers can acknowledge the same event at
//! different times. Studio's tail is a separate observation session and never
//! changes either.

mod base;
pub mod draft;
pub mod grammar;
pub use base::*;
