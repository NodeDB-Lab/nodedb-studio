//! Typed domain models shared across views. These describe *data* (collections,
//! databases, notifications); live UI state lives in `crate::state`.

pub mod admin;
pub mod array_data;
pub mod cluster;
pub mod collection;
pub mod connections;
pub mod cron;
pub mod designer;
pub mod explorer;
pub mod feeds;
pub mod format;
pub mod graph;
pub mod img_encode;
pub mod jobs;
pub mod json_text;
pub mod listen;
pub mod matviews;
pub mod notification;
pub mod records;
pub mod schema;
pub mod search;
pub mod shell;
pub mod spatial;
pub mod sync;
pub mod timeseries;
pub mod topics;
pub mod transfer;
pub mod vector;
pub mod workbench;
pub mod zone;
