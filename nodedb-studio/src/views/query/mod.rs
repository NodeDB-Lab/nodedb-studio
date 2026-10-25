//! The Query page: query tabs, the editor, results with plan and messages,
//! the schema, and the saved-query library.

pub mod details;
pub mod editor;
pub mod export;
pub mod library;
pub mod page;
pub mod params;
pub mod plan;
pub mod results;
pub mod schema;
pub mod tabs;
pub mod tasks;
pub mod toolbar;

pub use page::QueryPage;
