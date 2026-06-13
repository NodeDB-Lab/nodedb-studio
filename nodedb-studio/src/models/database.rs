//! A database within a NodeDB connection.
//!
//! `ActiveConnection` tracks databases as bare names (that's all the chip
//! popover and status bar need). This richer struct is the seam for when the
//! Explorer needs a database's collection listing.

use serde::{Deserialize, Serialize};

use crate::models::collection::Collection;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Database {
    pub name: String,
    pub collections: Vec<Collection>,
}
