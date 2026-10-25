//! The Designer's fixtures and the in-memory store that applies changes.

mod schema;
mod store;

#[cfg(test)]
pub use schema::design_schema;
pub use schema::explorer_schema;
pub use store::DesignerStore;
