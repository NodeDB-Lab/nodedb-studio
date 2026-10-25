//! Typed domain models shared across views. These describe *data* (collections,
//! databases, notifications); live UI state lives in `crate::state`.

pub mod admin;
pub mod array_data;
pub mod cdc;
pub mod cluster;
pub mod collection;
pub mod connections;
pub mod explorer;
pub mod format;
pub mod graph_records;
pub mod notification;
pub mod record_check;
pub mod record_json;
pub mod records;
pub mod schema;
pub mod shell;
pub mod streams;
pub mod transfer;
pub mod viewers;
pub mod workbench;
