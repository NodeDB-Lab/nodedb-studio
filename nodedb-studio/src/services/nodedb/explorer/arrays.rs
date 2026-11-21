//! Arrays have no source in NodeDB 0.6.2.
//!
//! The server describes an array nowhere: `DESCRIBE` answers not-found, there
//! is no `SHOW ARRAYS`, and `SHOW COLLECTIONS` lists only a name. `ARRAY_SLICE`
//! returns coordinates and attribute values with no dimension name, extent,
//! step, unit or attribute name, and it clips a range outside the extents
//! instead of refusing it. `ArraySchema` and `SliceResult` need all of those,
//! so both methods return a typed error instead of a guessed shape.

use crate::services::error::StudioError;

pub(in crate::services::nodedb) fn no_array_source(array: &str) -> StudioError {
    StudioError::Unsupported {
        what: format!(
            "describing array `{array}`: NodeDB 0.6.2 has no statement that reports an array's dimensions, extents or attributes"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_error_names_the_array() {
        let e = no_array_source("climate");
        assert!(matches!(e, StudioError::Unsupported { .. }), "{e:?}");
        assert!(e.to_string().contains("climate"));
    }
}
