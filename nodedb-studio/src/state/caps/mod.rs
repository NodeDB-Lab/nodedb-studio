//! Capability flags of one domain each.
//!
//! `Capabilities` in `state::connection` holds one field per domain, and
//! the type of that field lives here, in the domain's own file. A flag says
//! whether the connected server can serve a part of a screen. The mock
//! turns every flag on. The client-backed backend sets them for the server
//! it talks to.

pub mod cluster;
