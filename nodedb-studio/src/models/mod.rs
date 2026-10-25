//! Typed domain models shared across views. These describe *data* (collections,
//! databases, notifications); live UI state lives in `crate::state`.

pub mod admin;
pub mod array_data;
pub mod cdc;
pub mod cluster;
pub mod collection;
pub mod connections;
pub mod console;
pub mod designer;
pub mod designer_change;
pub mod designer_ddl;
pub mod designer_form;
pub mod designer_new;
pub mod explorer;
pub mod format;
pub mod graph_records;
pub mod notification;
pub mod query_plan;
pub mod record_check;
pub mod record_json;
pub mod records;
pub mod saved_query;
pub mod schema;
pub mod shell;
pub mod sql_text;
pub mod streams;
pub mod transfer;
pub mod viewers;
pub mod workbench;
