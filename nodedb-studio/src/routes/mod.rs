//! Studio routing: the route table, its page tabs, sidebar destinations, and
//! the shared shell layout.

pub mod destination;
pub mod layout;
pub mod route;
pub mod tabs;

pub use destination::{Destination, NAV_GROUPS};
pub use route::Route;
