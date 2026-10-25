//! The Console session: the protocol, the target, the transcript, and the
//! history filter.
//!
//! Provided by `App` above the router, so the transcript survives leaving
//! the page. The transaction is not here: it belongs to the server session,
//! which the Query page shares.

use crate::models::console::{ConsoleReply, HistoryEntry, HistoryWindow, Protocol};

/// One command and what it answered, in transcript order.
#[derive(Debug, Clone, PartialEq)]
pub struct ConsoleEntry {
    pub id: u32,
    pub protocol: Protocol,
    pub text: String,
    /// The reply, or the error message of the refusal.
    pub outcome: Result<ConsoleReply, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConsoleState {
    pub protocol: Protocol,
    /// The database commands run against. `None` follows the session's.
    pub database: Option<String>,
    /// The text in the prompt.
    pub prompt: String,
    pub entries: Vec<ConsoleEntry>,
    /// Whether a command is running.
    pub busy: bool,
    /// The history list's protocol filter. `None` lists every protocol.
    pub filter: Option<Protocol>,
    pub window: HistoryWindow,
    /// The history row the user picked.
    pub selected: Option<String>,
    /// Bumped when a command lands, so the history list reads again.
    pub epoch: u32,
    /// How far back Ctrl+Up has recalled, counted from the newest command of
    /// the protocol. `None` while the prompt holds its own text.
    recall: Option<usize>,
    next_id: u32,
}

impl Default for ConsoleState {
    fn default() -> Self {
        Self {
            protocol: Protocol::Sql,
            database: None,
            prompt: String::new(),
            entries: Vec::new(),
            busy: false,
            filter: Some(Protocol::Sql),
            window: HistoryWindow::AllTime,
            selected: None,
            epoch: 0,
            recall: None,
            next_id: 1,
        }
    }
}

impl ConsoleState {
    /// Switch protocol. The history filter follows, and a recall in
    /// progress ends.
    pub fn set_protocol(&mut self, protocol: Protocol) {
        self.protocol = protocol;
        self.filter = Some(protocol);
        self.recall = None;
    }

    /// Edit the prompt by hand, which ends a recall.
    pub fn edit_prompt(&mut self, text: String) {
        self.prompt = text;
        self.recall = None;
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Add a finished command to the transcript.
    pub fn push(&mut self, protocol: Protocol, text: &str, outcome: Result<ConsoleReply, String>) {
        let id = self.next_id;
        self.next_id += 1;
        self.entries.push(ConsoleEntry {
            id,
            protocol,
            text: text.trim().to_string(),
            outcome,
        });
        self.epoch += 1;
    }

    /// Step through the commands of the current protocol: `older` goes back,
    /// else forward. Walking forward past the newest restores an empty
    /// prompt. `history` is newest first.
    pub fn recall(&mut self, history: &[HistoryEntry], older: bool) {
        let mine: Vec<&HistoryEntry> = history
            .iter()
            .filter(|h| h.protocol == self.protocol)
            .collect();
        let next = match (self.recall, older) {
            (None, true) => Some(0),
            (None, false) => return,
            (Some(i), true) => Some((i + 1).min(mine.len().saturating_sub(1))),
            (Some(0), false) => None,
            (Some(i), false) => Some(i - 1),
        };
        self.recall = next;
        self.prompt = match next.and_then(|i| mine.get(i)) {
            Some(h) => h.text.clone(),
            None => String::new(),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::console::ConsoleOutput;
    use crate::models::workbench::TxState;

    fn entry(n: i64, protocol: Protocol, text: &str) -> HistoryEntry {
        HistoryEntry {
            id: format!("h{n}"),
            at: n,
            protocol,
            text: text.into(),
        }
    }

    fn reply() -> ConsoleReply {
        ConsoleReply {
            output: ConsoleOutput::Text(vec!["ok".into()]),
            elapsed_ms: 1,
            tx: TxState::Idle,
        }
    }

    #[test]
    fn a_command_joins_the_transcript_with_a_fresh_id() {
        let mut s = ConsoleState::default();
        s.push(Protocol::Sql, " SELECT 1; ", Ok(reply()));
        s.push(Protocol::Sql, "SELECT 2;", Err("boom".into()));
        let ids: Vec<u32> = s.entries.iter().map(|e| e.id).collect();
        assert_eq!(ids, [1, 2]);
        assert_eq!(s.entries[0].text, "SELECT 1;");
        assert_eq!(s.epoch, 2);
        s.clear();
        assert!(s.entries.is_empty());
        s.push(Protocol::Sql, "SELECT 3;", Ok(reply()));
        assert_eq!(s.entries[0].id, 3, "an id is never reused");
    }

    #[test]
    fn switching_protocol_moves_the_history_filter() {
        let mut s = ConsoleState::default();
        s.set_protocol(Protocol::Cypher);
        assert_eq!(s.filter, Some(Protocol::Cypher));
    }

    #[test]
    fn recall_walks_the_current_protocol_only() {
        let h = [
            entry(4, Protocol::Sql, "new"),
            entry(3, Protocol::Admin, "admin"),
            entry(2, Protocol::Sql, "mid"),
            entry(1, Protocol::Sql, "old"),
        ];
        let mut s = ConsoleState::default();
        s.recall(&h, false);
        assert_eq!(s.prompt, "", "forward with nothing recalled does nothing");
        s.recall(&h, true);
        assert_eq!(s.prompt, "new");
        s.recall(&h, true);
        assert_eq!(s.prompt, "mid");
        s.recall(&h, true);
        s.recall(&h, true);
        assert_eq!(s.prompt, "old", "the oldest command is the end");
        s.recall(&h, false);
        assert_eq!(s.prompt, "mid");
        s.recall(&h, false);
        s.recall(&h, false);
        assert_eq!(s.prompt, "", "past the newest the prompt is empty again");
    }

    #[test]
    fn typing_ends_a_recall() {
        let h = [entry(2, Protocol::Sql, "b"), entry(1, Protocol::Sql, "a")];
        let mut s = ConsoleState::default();
        s.recall(&h, true);
        s.edit_prompt("my own".into());
        s.recall(&h, true);
        assert_eq!(s.prompt, "b", "recall restarts from the newest");
    }
}
