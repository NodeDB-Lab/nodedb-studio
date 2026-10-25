//! Search: ranked retrieval over a collection's text and vector indexes, the
//! analyzer test, and the indexes with their build history. It replaces the
//! FTS Inspector.

mod analyze;
mod analyze_bodies;
mod compare;
mod hit_details;
mod hit_ranking;
mod index_details;
mod index_history;
mod index_list;
mod indexes;
mod page;
mod query_bar;
mod query_strip;
mod ranked;
mod rebuild_sheet;
mod results;
mod run_sheet;
mod tasks;

pub use page::SearchPage;
