//! Events & Jobs: change feeds, materialized views, topics, notifications,
//! and scheduled jobs.

pub mod common;
pub mod feed_details;
pub mod feed_editor;
pub mod feed_events;
pub mod feed_preview;
pub mod feed_tasks;
pub mod feeds;
pub mod job_editor;
pub mod job_review;
pub mod job_runs;
pub mod job_tasks;
pub mod jobs;
pub mod matviews;
pub mod mv_result;
pub mod mv_side;
pub mod mv_tasks;
pub mod notify;
pub mod notify_tasks;
pub mod page;
pub mod topic_groups;
pub mod topic_messages;
pub mod topic_sheets;
pub mod topic_tasks;
pub mod topics;

pub use page::EventsPage;
