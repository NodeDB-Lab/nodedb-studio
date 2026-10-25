//! Explorer: browse collections and their records, edit them, and move data
//! in and out.

mod actions;
mod array;
mod array_workspace;
mod catalog;
mod cells;
mod details;
mod details_geo;
mod details_rows;
mod details_vector;
mod graph;
mod links;
mod page;
mod schema_tabs;
mod table;
mod toolbar;
mod transfer_jobs;
mod workspace;

pub mod transfer;

pub use page::ExplorerPage;
pub use transfer::DataTransferPage;
