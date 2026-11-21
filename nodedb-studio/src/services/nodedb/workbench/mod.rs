//! Query workbench over the native client. Statements, transactions, plans,
//! the schema tree, and the Console reach the server. Saved queries and the
//! console history are Studio's own records, kept per saved connection and
//! user across reconnects.

mod base;
pub mod conn;
pub mod sql;
pub mod tx;
