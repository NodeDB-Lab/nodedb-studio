//! What the Designer draws: collections with their fields and indexes, and
//! the logical references between them.
//!
//! A collection is the Explorer catalog's `Collection` (one storage profile,
//! attached capabilities) plus its design: fields, indexes, and the settings
//! its profile carries. A reference is a design annotation. NodeDB does not
//! enforce a foreign key, so a reference never becomes constraint DDL.
//!
//! Where a change has been staged but not applied, `DesignSchema::with_pending`
//! lays it over the applied catalog as a draft.

mod base;
pub mod change;
pub mod ddl;
pub mod form;
pub mod new;
pub mod new_check;
pub use base::*;
