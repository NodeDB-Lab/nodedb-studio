//! Column-addressed decoding of tabular results into typed `models/` structs.
//!
//! Seam methods return Studio models, but the real implementation will build
//! them from a `QueryResult{columns, rows}`. `Table` is that shape expressed in
//! Studio's own terms, so decoders are unit-testable with no client or server.
//!
//! Every value arrives as a string on this path: `SHOW` and `DESCRIBE` return
//! timestamps, counts and booleans as strings, so decoders parse at the point
//! of use. That holds for the catalog and admin surface this module serves. It
//! does NOT hold for native `SELECT`, which since NodeDB [Unreleased] returns
//! nested objects and arrays as structured values rather than JSON text; those
//! results carry richer shapes than `Table` can hold and belong in
//! `models::workbench::ResultSet`, not here.

use crate::services::error::StudioError;

/// A tabular result: column names plus rows of stringly values.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

/// One row, addressable by column name.
#[allow(dead_code)] // SEAM-UNWIRED
pub struct Row<'a> {
    columns: &'a [String],
    cells: &'a [String],
}

impl<'a> Row<'a> {
    /// The cell under `name`, or an error naming the column that is missing.
    #[allow(dead_code)] // SEAM-UNWIRED
    pub fn field(&self, name: &str) -> Result<&'a str, StudioError> {
        let idx = self.columns.iter().position(|c| c == name).ok_or_else(|| {
            StudioError::UnexpectedColumns {
                expected: name.to_string(),
                got: self.columns.join(", "),
            }
        })?;
        self.cells
            .get(idx)
            .map(String::as_str)
            .ok_or_else(|| StudioError::UnexpectedColumns {
                expected: format!("row to have at least {} cells", idx + 1),
                got: format!("row has {} cells", self.cells.len()),
            })
    }
}

/// Decode every row of `table` with `f`, after asserting `expect` columns are
/// all present.
///
/// The assertion is the point: it turns the server's silent session-variable
/// fallback into a typed error rather than an empty result set.
#[allow(dead_code)] // SEAM-UNWIRED
pub fn decode_rows<T>(
    table: &Table,
    expect: &[&str],
    f: impl Fn(Row<'_>) -> Result<T, StudioError>,
) -> Result<Vec<T>, StudioError> {
    if let Some(missing) = expect
        .iter()
        .find(|e| !table.columns.iter().any(|c| c == *e))
    {
        return Err(StudioError::UnexpectedColumns {
            expected: format!("{} (missing: {missing})", expect.join(", ")),
            got: table.columns.join(", "),
        });
    }
    table
        .rows
        .iter()
        .map(|cells| {
            f(Row {
                columns: &table.columns,
                cells,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(cols: &[&str], rows: &[&[&str]]) -> Table {
        Table {
            columns: cols.iter().map(|s| s.to_string()).collect(),
            rows: rows
                .iter()
                .map(|r| r.iter().map(|s| s.to_string()).collect())
                .collect(),
        }
    }

    #[test]
    fn decodes_rows_by_column_name() {
        let t = table(&["name", "owner"], &[&["probe_docs", "admin"]]);
        let out = decode_rows(&t, &["name", "owner"], |r| {
            Ok(format!("{}/{}", r.field("name")?, r.field("owner")?))
        })
        .expect("decode must succeed");
        assert_eq!(out, vec!["probe_docs/admin".to_string()]);
    }

    #[test]
    fn column_order_does_not_matter() {
        let t = table(&["owner", "name"], &[&["admin", "probe_docs"]]);
        let out = decode_rows(&t, &["name", "owner"], |r| Ok(r.field("name")?.to_string()))
            .expect("decode must succeed");
        assert_eq!(out, vec!["probe_docs".to_string()]);
    }

    /// The server answers some SHOW statements with a session-variable
    /// fallback: cols=["setting"] and one empty row. That must be a typed
    /// error, never an empty list, or the screen renders "working but empty".
    #[test]
    fn setting_fallback_is_an_error_not_empty() {
        let t = table(&["setting"], &[&[""]]);
        let out = decode_rows(&t, &["name", "collection"], |r| {
            Ok(r.field("name")?.to_string())
        });
        assert!(
            matches!(out, Err(StudioError::UnexpectedColumns { .. })),
            "expected UnexpectedColumns, got {out:?}"
        );
    }

    #[test]
    fn genuinely_empty_result_is_ok_and_empty() {
        let t = table(&["name", "collection"], &[]);
        let out = decode_rows(&t, &["name", "collection"], |r| {
            Ok(r.field("name")?.to_string())
        })
        .expect("empty rows with correct columns is a valid empty result");
        assert!(out.is_empty());
    }

    #[test]
    fn missing_field_is_an_error() {
        let t = table(&["name"], &[&["x"]]);
        let out = decode_rows(&t, &["name"], |r| Ok(r.field("nope")?.to_string()));
        assert!(out.is_err());
    }

    #[test]
    fn short_row_is_an_error() {
        let t = table(&["name", "owner", "id"], &[&["probe_docs", "admin"]]);
        let out = decode_rows(&t, &["name", "owner", "id"], |r| {
            Ok(r.field("id")?.to_string())
        });
        assert!(
            matches!(out, Err(StudioError::UnexpectedColumns { .. })),
            "expected UnexpectedColumns for short row, got {out:?}"
        );
    }
}
