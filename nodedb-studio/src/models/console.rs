//! Console models: the three protocols, a command's reply, and history.

use crate::models::workbench::TxState;

/// How the Console talks to the server. Each protocol has its own grammar,
/// history, and syntax colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Protocol {
    #[default]
    Sql,
    Cypher,
    Admin,
}

impl Protocol {
    pub const ALL: [Protocol; 3] = [Protocol::Sql, Protocol::Cypher, Protocol::Admin];

    pub fn label(self) -> &'static str {
        match self {
            Protocol::Sql => "SQL",
            Protocol::Cypher => "Cypher",
            Protocol::Admin => "Admin",
        }
    }

    /// What the empty prompt asks for.
    pub fn placeholder(self) -> &'static str {
        match self {
            Protocol::Sql => "Enter a SQL statement",
            Protocol::Cypher => "Enter a Cypher query",
            Protocol::Admin => "Enter an admin command, such as SHOW DATABASES",
        }
    }
}

/// The result of one console command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsoleOutput {
    /// Rows under a header. Cells are display strings.
    Table {
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    /// Lines of text: a status tag or an admin report.
    Text(Vec<String>),
}

/// A command's reply, with the session's transaction state after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsoleReply {
    pub output: ConsoleOutput,
    pub elapsed_ms: u32,
    pub tx: TxState,
}

/// One command the session ran, newest first in the history list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub id: String,
    /// Seconds since the epoch, UTC.
    pub at: i64,
    pub protocol: Protocol,
    pub text: String,
}

/// How far back the history list reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HistoryWindow {
    #[default]
    AllTime,
    Day,
    Week,
}

impl HistoryWindow {
    pub const ALL: [HistoryWindow; 3] = [
        HistoryWindow::AllTime,
        HistoryWindow::Day,
        HistoryWindow::Week,
    ];

    pub fn label(self) -> &'static str {
        match self {
            HistoryWindow::AllTime => "All time",
            HistoryWindow::Day => "Last 24 hours",
            HistoryWindow::Week => "Last 7 days",
        }
    }

    /// The window in seconds. `None` reads everything.
    pub fn seconds(self) -> Option<i64> {
        match self {
            HistoryWindow::AllTime => None,
            HistoryWindow::Day => Some(86_400),
            HistoryWindow::Week => Some(7 * 86_400),
        }
    }
}

/// A history read: which protocol, and how far back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoryQuery {
    /// `None` lists every protocol.
    pub protocol: Option<Protocol>,
    pub window: HistoryWindow,
}
