//! Typed domain models shared across views. These describe *data* (collections,
//! databases, notifications); live UI state lives in `crate::state`.

pub mod admin;
pub mod cdc;
pub mod collection;
pub mod explorer;
pub mod notification;
pub mod streams;
pub mod viewers;
pub mod workbench;
