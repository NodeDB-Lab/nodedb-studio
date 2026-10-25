//! Small constructors the fixtures share.

use nodedb_types::{NdbDateTime, Value};

use crate::models::format::epoch;

pub fn s(text: &str) -> Value {
    Value::String(text.to_string())
}

pub fn int(n: i64) -> Value {
    Value::Integer(n)
}

/// A decimal as exact text, which is how Studio carries one.
pub fn dec(text: &str) -> Value {
    Value::String(text.to_string())
}

pub fn at(secs: i64) -> Value {
    // An epoch from `epoch()` is always in range.
    NdbDateTime::from_secs(secs)
        .map(Value::DateTime)
        .unwrap_or(Value::Null)
}

/// A UTC timestamp value.
pub fn ts(y: i64, mo: u32, d: u32, h: u32, mi: u32, sec: u32) -> Value {
    at(epoch(y, mo, d, h, mi, sec))
}

/// The instant the fixtures call now: the Explorer's observation time.
pub const NOW: i64 = epoch(2026, 6, 13, 13, 40, 0);

/// A deterministic float in `[0, 1)` from an index and a salt.
pub fn unit(i: usize, salt: u64) -> f64 {
    let mut x =
        (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 30;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 27;
    (x >> 11) as f64 / (1u64 << 53) as f64
}

use crate::models::records::Record;

/// A collection's records, produced one at a time by index. Index order is
/// the collection's natural order.
#[derive(Clone, Copy)]
pub struct Dataset {
    pub total: usize,
    pub row: fn(usize) -> Record,
}
