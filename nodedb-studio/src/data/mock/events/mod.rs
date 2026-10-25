//! Events & Jobs fixtures: one file per tab.

pub mod feeds;
pub mod jobs;
pub mod listen;
pub mod matviews;
pub mod topics;

#[cfg(test)]
pub use feeds::seeded_feeds;
pub use feeds::{FeedStore, preview as preview_feed};
pub use jobs::JobStore;
#[cfg(test)]
pub use jobs::seeded_jobs;
pub use listen::{ListenStore, seen_messages};
pub use matviews::ViewStore;
pub use topics::TopicStore;
