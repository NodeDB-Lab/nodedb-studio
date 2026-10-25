//! Events & Jobs UI state, one file per tab, and the bundle `App` provides.

pub mod feeds;
pub mod jobs;
pub mod matviews;
pub mod notify;
pub mod topics;
pub mod ui;

#[cfg(all(feature = "screenshots", target_os = "linux"))]
pub use ui::EventsSeed;
pub use ui::EventsUi;
