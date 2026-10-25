//! The Console: a protocol, a target database, the session's transaction,
//! a transcript, a multiline prompt, and the command history.

pub mod history;
pub mod page;
pub mod tasks;
pub mod toolbar;
pub mod transcript;

pub use page::ConsolePage;
