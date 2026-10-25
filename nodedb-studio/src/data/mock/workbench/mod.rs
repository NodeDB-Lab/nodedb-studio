//! Workbench fixtures: results, plans, the schema tree, the saved-query
//! library, and the console. `store` holds the state writes change.

mod console;
mod library;
mod plans;
mod results;
mod schema;
mod store;

pub use library::query_session;
#[cfg(any(test, all(feature = "screenshots", target_os = "linux")))]
pub use library::{CUSTOMER_SPEND_ID, DATABASE, ORDERS_SQL, library};
pub use plans::query_plan;
pub use results::empty_result_set;
pub use schema::schema_tree;
pub use store::WorkbenchStore;
