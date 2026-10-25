//! Events & Jobs fixtures: one file per tab.

mod feed_history;
mod feed_preview;
pub mod feeds;
pub mod jobs;
pub mod listen;
pub mod matviews;
pub mod topics;

pub use feed_preview::preview as preview_feed;
pub use feeds::FeedStore;
#[cfg(test)]
pub use feeds::seeded_feeds;
pub use jobs::JobStore;
#[cfg(test)]
pub use jobs::seeded_jobs;
pub use listen::ListenStore;
#[cfg(all(feature = "screenshots", target_os = "linux"))]
pub use listen::seen_messages;
pub use matviews::ViewStore;
pub use topics::TopicStore;
