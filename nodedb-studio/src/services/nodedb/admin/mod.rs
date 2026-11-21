//! Administration reads and changes over `SHOW` and DDL statements.
//!
//! Wired: the access directory and subject access, the policy list, the
//! audit trail and its filter lists, and access changes. Every other
//! method returns a typed error that names what NodeDB 0.6.2 lacks (see
//! `unavailable`).
//!
//! The server has no revision on users or roles. A review's revision is a
//! hash of the rows the change read, and apply reads them again and refuses
//! when the hash differs. Another administrator's change that lands between
//! that read and the statement is not caught.

pub mod access;
pub mod audit;
mod base;
pub mod grants;
pub mod plan;
pub mod policy;
pub mod user;
