//! Materialized views: a saved query whose result the server keeps and
//! refreshes. Browsing stays in Explorer and the definition belongs to
//! Designer, so a view here links to both by its stable id.

use serde::{Deserialize, Serialize};

use crate::models::feeds::Tone;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RefreshPolicy {
    /// Only the changes since the watermark are applied.
    Incremental,
    /// The whole result is recomputed.
    Full,
}

impl RefreshPolicy {
    pub fn label(self) -> &'static str {
        match self {
            RefreshPolicy::Incremental => "Incremental",
            RefreshPolicy::Full => "Full",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RefreshStatus {
    Completed,
    Running,
    Error,
}

impl RefreshStatus {
    pub fn label(self) -> &'static str {
        match self {
            RefreshStatus::Completed => "Completed",
            RefreshStatus::Running => "Running",
            RefreshStatus::Error => "Error",
        }
    }

    pub fn tone(self) -> Tone {
        match self {
            RefreshStatus::Completed => Tone::Ok,
            RefreshStatus::Running => Tone::Warn,
            RefreshStatus::Error => Tone::Err,
        }
    }
}

/// What started a refresh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Trigger {
    Scheduled,
    Manual,
}

impl Trigger {
    pub fn label(self) -> &'static str {
        match self {
            Trigger::Scheduled => "Scheduled",
            Trigger::Manual => "Manual",
        }
    }
}

/// What the server reports about a refresh that failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshError {
    pub message: String,
    /// What the server names as the failing source, if it does.
    pub source: Option<String>,
    /// What to do next, as the server words it.
    pub hint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshRun {
    pub id: String,
    /// `2026-06-13 13:39:00`, UTC.
    pub at: String,
    pub status: RefreshStatus,
    /// `Processed 18,432 rows (12.4 MB)`, or the error's one line.
    pub detail: String,
    pub started_by: Trigger,
    pub error: Option<RefreshError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaterializedView {
    /// Stable identity. The Designer and the Explorer open the view by it.
    pub id: String,
    pub name: String,
    pub database: String,
    pub definition: String,
    pub policy: RefreshPolicy,
    /// The source position the last refresh applied through, UTC.
    pub watermark: Option<String>,
    /// How far the view trails its sources, when the server reports it.
    pub lag_secs: Option<u64>,
    /// The objects the definition reads, as `database.name`.
    pub dependencies: Vec<String>,
    pub row_count: u64,
}

impl MaterializedView {
    /// `Not reported` until the server reports a lag.
    pub fn lag_label(&self) -> String {
        match self.lag_secs {
            Some(s) => crate::models::format::duration(s),
            None => "Not reported".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewColumn {
    pub name: String,
    pub numeric: bool,
}

/// One page of a view's rows. Cells are display text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewRows {
    pub columns: Vec<ViewColumn>,
    /// `(stable key, cells)`.
    pub rows: Vec<(String, Vec<String>)>,
    pub total: u64,
    pub page: usize,
    pub page_size: usize,
}

impl ViewRows {
    pub fn pages(&self) -> usize {
        (self.total as usize).div_ceil(self.page_size.max(1)).max(1)
    }

    /// `Rows 1–10 of 90`.
    pub fn range_label(&self) -> String {
        if self.rows.is_empty() {
            return "No rows".to_string();
        }
        let from = self.page * self.page_size + 1;
        let to = from + self.rows.len() - 1;
        format!("Rows {from}–{to} of {}", self.total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(total: u64, page: usize, n: usize) -> ViewRows {
        ViewRows {
            columns: Vec::new(),
            rows: (0..n).map(|i| (i.to_string(), Vec::new())).collect(),
            total,
            page,
            page_size: 10,
        }
    }

    #[test]
    fn pages_round_up_and_never_reach_zero() {
        assert_eq!(rows(90, 0, 10).pages(), 9);
        assert_eq!(rows(91, 0, 10).pages(), 10);
        assert_eq!(rows(0, 0, 0).pages(), 1);
    }

    #[test]
    fn the_range_label_counts_from_one() {
        assert_eq!(rows(90, 0, 10).range_label(), "Rows 1–10 of 90");
        assert_eq!(rows(90, 8, 10).range_label(), "Rows 81–90 of 90");
        assert_eq!(rows(0, 0, 0).range_label(), "No rows");
    }
}
