//! Workbench models: query requests, result sets, transactions, and the
//! schema tree.

use serde::Serialize;

use crate::models::collection::Profile;
use crate::models::explorer::RecordRow;
use crate::models::format::thousands;
use crate::services::error::StudioError;

/// One page of query output. Pagination is the seam's responsibility: the
/// client buffers whole result sets, so the real implementation emits
/// LIMIT/OFFSET rather than holding a cursor.
///
/// Rectangular and uniquely keyed by construction: every row has exactly
/// `columns.len()` cells and a distinct `id`. Build one through
/// [`ResultSet::new`], which rejects a ragged row or a duplicate id with
/// `MalformedRows`, so a decoder that drops a cell surfaces as an error
/// instead of a grid whose values sit under the wrong headers. `cells` hold display strings; sorting
/// or typed comparison needs raw values the model does not carry yet.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ResultSet {
    columns: Vec<String>,
    rows: Vec<RecordRow>,
    pub elapsed_ms: u32,
    /// Rows the engine read, as the server reports it (`14M`). Empty when
    /// the server does not say.
    pub scanned: String,
    /// How the engine reached the rows (`index_scan(events_ts_idx)`). Empty
    /// when the server does not say.
    pub access_path: String,
    /// Rows the whole statement produced, across every page.
    pub total_rows: usize,
    /// The zero-based page `rows` holds.
    pub page: usize,
    /// Rows per page the request asked for.
    pub page_size: usize,
}

impl ResultSet {
    /// Build a result set, rejecting any row whose width differs from the
    /// header and any duplicate row id. It holds one page with every row.
    pub fn new(
        columns: Vec<String>,
        rows: Vec<RecordRow>,
        elapsed_ms: u32,
        scanned: String,
    ) -> Result<Self, StudioError> {
        let width = columns.len();
        if let Some(bad) = rows.iter().find(|r| r.cells.len() != width) {
            return Err(StudioError::MalformedRows {
                reason: format!(
                    "row {} has {} cells, expected {width}",
                    bad.id,
                    bad.cells.len()
                ),
            });
        }
        // Rows are Dioxus sibling keys; a duplicate panics in debug and
        // silently reuses the wrong node in release.
        let mut seen = std::collections::HashSet::with_capacity(rows.len());
        if let Some(dup) = rows.iter().find(|r| !seen.insert(r.id.as_str())) {
            return Err(StudioError::MalformedRows {
                reason: format!("duplicate row id {}", dup.id),
            });
        }
        let total_rows = rows.len();
        Ok(Self {
            columns,
            rows,
            elapsed_ms,
            scanned,
            access_path: String::new(),
            total_rows,
            page: 0,
            page_size: total_rows.max(1),
        })
    }

    /// Name the access path the engine took.
    pub fn with_access_path(mut self, path: &str) -> Self {
        self.access_path = path.to_string();
        self
    }

    /// Mark these rows as page `page` of `total_rows`, `page_size` to a page.
    pub fn with_page(mut self, page: usize, page_size: usize, total_rows: usize) -> Self {
        self.page = page;
        self.page_size = page_size.max(1);
        self.total_rows = total_rows;
        self
    }

    pub fn columns(&self) -> &[String] {
        &self.columns
    }

    pub fn rows(&self) -> &[RecordRow] {
        &self.rows
    }

    /// How many pages the whole statement fills. At least one.
    pub fn pages(&self) -> usize {
        self.total_rows.div_ceil(self.page_size).max(1)
    }

    /// The one-based range of rows this page shows, `None` when it is empty.
    pub fn shown(&self) -> Option<(usize, usize)> {
        if self.rows.is_empty() {
            return None;
        }
        let first = self.page * self.page_size + 1;
        Some((first, first + self.rows.len() - 1))
    }

    /// Whether every cell of column `i` reads as a number, so the grid can
    /// right-align it. A dash is a missing value and does not decide.
    pub fn is_numeric(&self, i: usize) -> bool {
        let mut seen = false;
        for row in &self.rows {
            let Some(cell) = row.cells.get(i) else {
                return false;
            };
            let cell = cell.trim();
            if cell.is_empty() || cell == "-" || cell == "\u{2013}" || cell == "\u{2014}" {
                continue;
            }
            if !cell
                .chars()
                .all(|c| c.is_ascii_digit() || matches!(c, ',' | '.' | '-'))
            {
                return false;
            }
            seen = true;
        }
        seen
    }
}

/// The type of a statement parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParamType {
    #[default]
    Text,
    Integer,
    Decimal,
    Boolean,
    Timestamp,
}

impl ParamType {
    pub const ALL: [ParamType; 5] = [
        ParamType::Text,
        ParamType::Integer,
        ParamType::Decimal,
        ParamType::Boolean,
        ParamType::Timestamp,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ParamType::Text => "text",
            ParamType::Integer => "integer",
            ParamType::Decimal => "decimal",
            ParamType::Boolean => "boolean",
            ParamType::Timestamp => "timestamp",
        }
    }
}

/// A value that does not fit its parameter's type.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParamError {
    #[error("parameter :{name} needs a value")]
    Missing { name: String },
    #[error("parameter :{name} is {expected}: `{value}` is not")]
    Mismatch {
        name: String,
        expected: &'static str,
        value: String,
    },
}

/// One `:name` parameter with its type and the value to bind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamBinding {
    pub name: String,
    pub ty: ParamType,
    pub value: String,
}

impl ParamBinding {
    pub fn new(name: &str, ty: ParamType, value: &str) -> Self {
        Self {
            name: name.to_string(),
            ty,
            value: value.to_string(),
        }
    }

    /// Whether the value can bind as the declared type.
    pub fn check(&self) -> Result<(), ParamError> {
        let v = self.value.trim();
        if v.is_empty() {
            return Err(ParamError::Missing {
                name: self.name.clone(),
            });
        }
        let ok = match self.ty {
            ParamType::Text => true,
            ParamType::Integer => v.parse::<i64>().is_ok(),
            ParamType::Decimal => v.parse::<f64>().is_ok_and(f64::is_finite),
            ParamType::Boolean => matches!(v, "true" | "false"),
            ParamType::Timestamp => is_timestamp(v),
        };
        if ok {
            Ok(())
        } else {
            Err(ParamError::Mismatch {
                name: self.name.clone(),
                expected: match self.ty {
                    ParamType::Text => "text",
                    ParamType::Integer => "an integer",
                    ParamType::Decimal => "a decimal",
                    ParamType::Boolean => "true or false",
                    ParamType::Timestamp => "a timestamp like 2026-06-13 12:48:21",
                },
                value: v.to_string(),
            })
        }
    }
}

/// `YYYY-MM-DD`, optionally followed by `HH:MM[:SS]`.
fn is_timestamp(v: &str) -> bool {
    let (date, time) = match v.split_once([' ', 'T']) {
        Some((d, t)) => (d, Some(t)),
        None => (v, None),
    };
    let d: Vec<&str> = date.split('-').collect();
    let date_ok = d.len() == 3
        && d[0].len() == 4
        && d[1].len() == 2
        && d[2].len() == 2
        && d.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()))
        && matches!(d[1].parse::<u32>(), Ok(1..=12))
        && matches!(d[2].parse::<u32>(), Ok(1..=31));
    let time_ok = time.is_none_or(|t| {
        let p: Vec<&str> = t.trim_end_matches('Z').split(':').collect();
        (p.len() == 2 || p.len() == 3)
            && p.iter()
                .all(|x| x.len() == 2 && x.chars().all(|c| c.is_ascii_digit()))
            && matches!(p[0].parse::<u32>(), Ok(0..=23))
            && matches!(p[1].parse::<u32>(), Ok(0..=59))
    });
    date_ok && time_ok
}

/// What to run, and which page of its rows to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryRequest {
    pub sql: String,
    pub params: Vec<ParamBinding>,
    /// The database the statement is bound to.
    pub database: String,
    pub page: usize,
    pub page_size: usize,
}

/// The default rows per page.
pub const PAGE_SIZE: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageLevel {
    Info,
    Warning,
}

/// A server notice that came with a statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryMessage {
    pub level: MessageLevel,
    pub text: String,
}

/// A finished statement: its rows, its notices, and the transaction it ran in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryRun {
    pub result: ResultSet,
    pub messages: Vec<QueryMessage>,
    pub tx: TxState,
}

/// What a transaction may do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxAccess {
    ReadOnly,
    ReadWrite,
}

impl TxAccess {
    pub fn label(self) -> &'static str {
        match self {
            TxAccess::ReadOnly => "read-only",
            TxAccess::ReadWrite => "read-write",
        }
    }
}

/// The session's transaction. The server reports `Idle`, in a block, or in
/// a failed block, which only a rollback leaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TxState {
    #[default]
    Idle,
    Active {
        access: TxAccess,
        /// Statements run since BEGIN.
        statements: u32,
    },
    Failed,
}

impl TxState {
    /// `Idle`, `Active (read-only)`, `Active (read-write)`, `Failed`.
    pub fn label(self) -> String {
        match self {
            TxState::Idle => "Idle".to_string(),
            TxState::Active { access, .. } => format!("Active ({})", access.label()),
            TxState::Failed => "Failed".to_string(),
        }
    }

    /// Whether Commit and Rollback apply.
    pub fn is_open(self) -> bool {
        !matches!(self, TxState::Idle)
    }
}

/// A query tab as the session restores it. `saved_sql` is the text last
/// stored: a tab whose `sql` differs from it has unsaved changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryDraft {
    pub id: String,
    pub title: String,
    pub sql: String,
    pub saved_sql: String,
    /// The saved query this tab is bound to, if any.
    pub saved_query: Option<String>,
    pub database: String,
}

/// What a schema entry is, as its letter badge shows. A collection has a
/// storage profile. Vector and Graph entries are the typed attributes and
/// edge sets the schema lists beside them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaKind {
    Collection(Profile),
    Vector,
    Graph,
    MaterializedView,
}

impl SchemaKind {
    pub fn letter(self) -> &'static str {
        match self {
            SchemaKind::Collection(Profile::Document) => "D",
            SchemaKind::Collection(Profile::DocumentStrict) => "S",
            SchemaKind::Collection(Profile::KeyValue) => "K",
            SchemaKind::Collection(Profile::Columnar) => "C",
            SchemaKind::Collection(Profile::Timeseries) => "T",
            SchemaKind::Collection(Profile::Spatial) => "L",
            SchemaKind::Collection(Profile::Array) => "A",
            SchemaKind::Vector => "V",
            SchemaKind::Graph => "G",
            SchemaKind::MaterializedView => "M",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SchemaKind::Collection(p) => p.storage_label(),
            SchemaKind::Vector => "Vector",
            SchemaKind::Graph => "Graph",
            SchemaKind::MaterializedView => "Materialized view",
        }
    }

    /// The badge tone class.
    pub fn tone(self) -> &'static str {
        match self {
            SchemaKind::Collection(Profile::Document) => "blue",
            SchemaKind::Collection(Profile::DocumentStrict) => "amber",
            SchemaKind::Collection(Profile::KeyValue) => "teal",
            SchemaKind::Collection(Profile::Columnar) => "gray",
            SchemaKind::Collection(Profile::Timeseries) => "green",
            SchemaKind::Collection(Profile::Spatial) => "violet",
            SchemaKind::Collection(Profile::Array) => "gray",
            SchemaKind::Vector => "violet",
            SchemaKind::Graph => "orange",
            SchemaKind::MaterializedView => "blue",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaField {
    pub name: String,
    pub type_name: String,
    pub key: bool,
}

/// A collection or view and its fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaTable {
    pub name: String,
    pub kind: SchemaKind,
    /// Rows the table holds, when the server counts them.
    pub rows: Option<u64>,
    pub fields: Vec<SchemaField>,
}

impl SchemaTable {
    /// `2.4M`, `12,481`, or empty when the count is unknown.
    pub fn count_label(&self) -> String {
        self.rows.map(compact).unwrap_or_default()
    }
}

/// `2400000` becomes `2.4M`. Counts under a million keep their digits.
fn compact(n: u64) -> String {
    if n >= 1_000_000 {
        let tenths = (n + 50_000) / 100_000;
        format!("{}.{}M", tenths / 10, tenths % 10)
    } else {
        thousands(n)
    }
}

/// The tables and views the Schema panel lists.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SchemaTree {
    pub tables: Vec<SchemaTable>,
    pub views: Vec<SchemaTable>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, cells: &[&str]) -> RecordRow {
        RecordRow {
            id: id.into(),
            cells: cells.iter().map(|c| c.to_string()).collect(),
        }
    }

    #[test]
    fn new_accepts_rectangular_rows() {
        let rs = ResultSet::new(
            vec!["a".into(), "b".into()],
            vec![row("1", &["x", "y"]), row("2", &["p", "q"])],
            0,
            String::new(),
        );
        assert!(rs.is_ok());
    }

    #[test]
    fn new_rejects_a_short_row() {
        let rs = ResultSet::new(
            vec!["a".into(), "b".into()],
            vec![row("1", &["x", "y"]), row("2", &["only-one"])],
            0,
            String::new(),
        );
        assert!(matches!(rs, Err(StudioError::MalformedRows { .. })));
    }

    #[test]
    fn new_rejects_a_duplicate_row_id() {
        let rs = ResultSet::new(
            vec!["a".into()],
            vec![row("same", &["x"]), row("same", &["y"])],
            0,
            String::new(),
        );
        assert!(matches!(rs, Err(StudioError::MalformedRows { .. })));
    }

    /// The message names the rows, not the columns, so the user looks at
    /// the right fault.
    #[test]
    fn rejection_message_names_the_offending_row() {
        let err = ResultSet::new(
            vec!["a".into(), "b".into()],
            vec![row("r2", &["only-one"])],
            0,
            String::new(),
        )
        .expect_err("ragged row");
        assert_eq!(
            err.to_string(),
            "result rows do not match their header: row r2 has 1 cells, expected 2"
        );
    }

    #[test]
    fn new_rejects_a_long_row() {
        let rs = ResultSet::new(
            vec!["a".into()],
            vec![row("1", &["x", "extra"])],
            0,
            String::new(),
        );
        assert!(matches!(rs, Err(StudioError::MalformedRows { .. })));
    }
}

#[cfg(test)]
mod more_tests {
    use super::*;

    fn row(id: &str, cells: &[&str]) -> RecordRow {
        RecordRow {
            id: id.into(),
            cells: cells.iter().map(|c| c.to_string()).collect(),
        }
    }

    #[test]
    fn compact_counts_match_the_schema_panel() {
        assert_eq!(compact(2_400_000), "2.4M");
        assert_eq!(compact(1_100_000), "1.1M");
        assert_eq!(compact(12_481), "12,481");
        assert_eq!(compact(442_003), "442,003");
    }

    #[test]
    fn pages_round_up_and_shown_is_one_based() {
        let rs = ResultSet::new(
            vec!["a".into()],
            (0..5).map(|i| row(&format!("r{i}"), &["x"])).collect(),
            1,
            String::new(),
        )
        .expect("rectangular")
        .with_page(1, 5, 127);
        assert_eq!(rs.pages(), 26);
        assert_eq!(rs.shown(), Some((6, 10)));
    }

    #[test]
    fn numeric_columns_ignore_dashes() {
        let rs = ResultSet::new(
            vec!["t".into(), "n".into()],
            vec![row("1", &["a", "14,820"]), row("2", &["b", "\u{2014}"])],
            0,
            String::new(),
        )
        .expect("rectangular");
        assert!(!rs.is_numeric(0));
        assert!(rs.is_numeric(1));
    }

    #[test]
    fn a_parameter_must_fit_its_type() {
        let p = |ty, v| ParamBinding::new("x", ty, v).check();
        assert!(p(ParamType::Text, "USD").is_ok());
        assert!(p(ParamType::Integer, "42").is_ok());
        assert!(matches!(
            p(ParamType::Integer, "4.2"),
            Err(ParamError::Mismatch { .. })
        ));
        assert!(p(ParamType::Decimal, "4.2").is_ok());
        assert!(p(ParamType::Boolean, "true").is_ok());
        assert!(p(ParamType::Boolean, "yes").is_err());
        assert!(p(ParamType::Timestamp, "2026-06-13 12:48:21").is_ok());
        assert!(p(ParamType::Timestamp, "2026-13-01").is_err());
        assert!(matches!(
            p(ParamType::Text, "  "),
            Err(ParamError::Missing { .. })
        ));
    }

    #[test]
    fn transaction_labels() {
        assert_eq!(TxState::Idle.label(), "Idle");
        let active = TxState::Active {
            access: TxAccess::ReadOnly,
            statements: 0,
        };
        assert_eq!(active.label(), "Active (read-only)");
        assert!(active.is_open());
        assert!(!TxState::Idle.is_open());
    }
}
