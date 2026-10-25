//! Import and export jobs.
//!
//! A job runs on the server. Studio starts it, lists it, and cancels it. It
//! never claims a job finished before the server says so.

use crate::models::records::Filter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Jsonl,
    Csv,
    Parquet,
}

impl ExportFormat {
    pub const ALL: [ExportFormat; 3] = [
        ExportFormat::Jsonl,
        ExportFormat::Csv,
        ExportFormat::Parquet,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ExportFormat::Jsonl => "JSONL",
            ExportFormat::Csv => "CSV",
            ExportFormat::Parquet => "Parquet",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Jsonl => "jsonl",
            ExportFormat::Csv => "csv",
            ExportFormat::Parquet => "parquet",
        }
    }

    /// Whether the delimiter, header, and encoding options apply.
    pub fn delimited(self) -> bool {
        match self {
            ExportFormat::Csv => true,
            ExportFormat::Jsonl | ExportFormat::Parquet => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delimiter {
    Comma,
    Tab,
    Semicolon,
    Pipe,
}

impl Delimiter {
    pub const ALL: [Delimiter; 4] = [
        Delimiter::Comma,
        Delimiter::Tab,
        Delimiter::Semicolon,
        Delimiter::Pipe,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Delimiter::Comma => "Comma (,)",
            Delimiter::Tab => "Tab",
            Delimiter::Semicolon => "Semicolon (;)",
            Delimiter::Pipe => "Pipe (|)",
        }
    }

    /// The word the summary line uses.
    pub fn word(self) -> &'static str {
        match self {
            Delimiter::Comma => "comma",
            Delimiter::Tab => "tab",
            Delimiter::Semicolon => "semicolon",
            Delimiter::Pipe => "pipe",
        }
    }
}

/// Which rows an export reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportScope {
    All,
    /// The rows the Explorer's filter matches.
    Filtered(Filter),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportRequest {
    pub collection: String,
    pub format: ExportFormat,
    pub scope: ExportScope,
    pub fields: Vec<String>,
    pub header: bool,
    pub delimiter: Delimiter,
}

/// The first rows an export would write, and how many it would write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportPreview {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub total: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobKind {
    Export,
    Import,
}

impl JobKind {
    pub fn label(self) -> &'static str {
        match self {
            JobKind::Export => "Export",
            JobKind::Import => "Import",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobState {
    Running { percent: u8 },
    Completed,
    Cancelled,
    Failed { reason: String },
}

impl JobState {
    pub fn label(&self) -> &'static str {
        match self {
            JobState::Running { .. } => "Running",
            JobState::Completed => "Completed",
            JobState::Cancelled => "Cancelled",
            JobState::Failed { .. } => "Failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferJob {
    pub id: String,
    /// The file the job reads or writes, `users.csv`.
    pub name: String,
    pub kind: JobKind,
    pub collection: String,
    pub state: JobState,
    /// Seconds since the epoch.
    pub started_at: i64,
}

impl TransferJob {
    pub fn running(&self) -> bool {
        matches!(self.state, JobState::Running { .. })
    }
}
