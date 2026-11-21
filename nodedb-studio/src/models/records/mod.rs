//! Records, and the queries and pages the Explorer reads them through.
//!
//! A record's values are `nodedb_types::Value`, as the server returns them.
//! Studio keeps field order from the schema, never from the value: an
//! object's own key order is not stable.

mod base;
pub mod check;
pub mod json;
pub use base::*;
