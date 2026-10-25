//! Timeseries: Explore, Dashboards, and Lifecycle over user collections.

mod actions;
mod backfill;
mod backfill_ops;
mod backfill_panels;
mod backfill_review;
mod backfill_tasks;
mod chart;
mod dash_bars;
mod dash_dialogs;
mod dash_ops;
mod dash_panel;
mod dash_tasks;
mod dashboards;
mod explore;
mod explore_query;
mod explore_table;
mod explore_tasks;
mod late_data;
mod library;
mod life_ops;
mod life_retention;
mod life_review;
mod life_sheets;
mod life_tasks;
mod lifecycle;
mod notice;
mod page;
mod panel_editor;
mod picker;
mod retention;
mod rollup_side;
mod toolbar;

pub use page::TimeseriesPage;
