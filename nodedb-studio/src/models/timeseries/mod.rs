//! Timeseries exploration: what a user asks of a collection over time, and
//! the shape of the answer.
//!
//! A query names a collection, an event-time field, one measure, a bucket,
//! an optional grouping field, filters, and a half-open UTC range
//! `[start, end)`. The answer is a set of series of buckets. A bucket the
//! source has no rows for is absent, never a zero: only an explicit gap
//! policy adds points, and each added point says it was filled.

pub mod agg;
pub mod axis;
pub mod backfill;
mod base;
pub mod chart;
pub mod dashboard;
pub mod lifecycle;
pub mod result;
pub mod rollup;
pub mod rollup_draft;
pub mod source;
pub use base::*;
