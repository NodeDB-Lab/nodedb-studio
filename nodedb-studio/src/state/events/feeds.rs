//! Change feeds page state: the selection, Studio's tail, and the editor.
//!
//! The tail is a Studio observation session. Its controls change what this
//! window shows and never what the server delivers.

use crate::models::feed_draft::FeedDraft;
use crate::models::feeds::{FeedEvent, FeedSession};

/// The most events the tail holds on screen. The oldest drop first.
pub const TAIL_LIMIT: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FeedTab {
    #[default]
    Definition,
    Consumers,
    Activity,
}

impl FeedTab {
    pub const ALL: [FeedTab; 3] = [FeedTab::Definition, FeedTab::Consumers, FeedTab::Activity];

    pub fn label(self) -> &'static str {
        match self {
            FeedTab::Definition => "Definition",
            FeedTab::Consumers => "Consumers",
            FeedTab::Activity => "Activity",
        }
    }
}

/// Studio's observation of one feed.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Tail {
    /// The feed the buffer belongs to.
    pub feed: Option<String>,
    /// The Studio consumer group, while the tail runs.
    pub session: Option<FeedSession>,
    /// Display frozen. Events keep arriving into `held`.
    pub paused: bool,
    /// Events shown, newest first.
    pub shown: Vec<FeedEvent>,
    /// Events that arrived while the display was paused, newest first.
    pub held: Vec<FeedEvent>,
    /// Events dropped because the held buffer filled.
    pub skipped: usize,
    /// Why the last read failed. The events already shown stay, marked stale.
    pub error: Option<String>,
}

impl Tail {
    pub fn running(&self) -> bool {
        self.session.is_some()
    }

    /// A tail that belongs to `feed`.
    pub fn is_for(&self, feed: &str) -> bool {
        self.feed.as_deref() == Some(feed)
    }

    /// Add events a read delivered, oldest first. A paused display holds them.
    pub fn receive(&mut self, events: Vec<FeedEvent>) {
        let target = if self.paused {
            &mut self.held
        } else {
            &mut self.shown
        };
        for e in events {
            if target.iter().any(|x| x.id == e.id) {
                continue;
            }
            target.insert(0, e);
        }
        if self.paused {
            if self.held.len() > TAIL_LIMIT {
                self.skipped += self.held.len() - TAIL_LIMIT;
                self.held.truncate(TAIL_LIMIT);
            }
        } else {
            self.shown.truncate(TAIL_LIMIT);
        }
    }

    /// Resume the display: the held events join the shown ones.
    pub fn resume(&mut self) {
        self.paused = false;
        let held = std::mem::take(&mut self.held);
        for e in held.into_iter().rev() {
            if !self.shown.iter().any(|x| x.id == e.id) {
                self.shown.insert(0, e);
            }
        }
        self.shown.truncate(TAIL_LIMIT);
    }

    /// Start observing `feed` with `session`. The old buffer goes.
    pub fn start(&mut self, feed: &str, session: FeedSession) {
        *self = Tail {
            feed: Some(feed.to_string()),
            session: Some(session),
            ..Tail::default()
        };
    }

    /// Stop observing. The events already shown stay on screen.
    pub fn stop(&mut self) {
        self.session = None;
        self.paused = false;
        self.resume();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EditorMode {
    Create,
    /// Edit feed `id`, read at `revision`.
    Edit {
        id: String,
        revision: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorStep {
    #[default]
    Form,
    Review,
}

/// The Create and Edit change feed drawer.
#[derive(Debug, Clone, PartialEq)]
pub struct FeedEditor {
    pub mode: EditorMode,
    pub draft: FeedDraft,
    pub step: EditorStep,
    /// Review feed was pressed, so the form shows every issue.
    pub tried: bool,
    pub busy: bool,
    /// The server's reason when the save failed.
    pub error: Option<String>,
    /// Event schema nodes the preview has open, by path.
    pub open_nodes: Vec<String>,
}

impl FeedEditor {
    pub fn create(source: &str) -> Self {
        Self::with(EditorMode::Create, FeedDraft::blank(source))
    }

    pub fn edit(id: &str, revision: u64, draft: FeedDraft) -> Self {
        Self::with(
            EditorMode::Edit {
                id: id.to_string(),
                revision,
            },
            draft,
        )
    }

    pub fn with(mode: EditorMode, draft: FeedDraft) -> Self {
        Self {
            mode,
            draft,
            step: EditorStep::Form,
            tried: false,
            busy: false,
            error: None,
            open_nodes: vec!["ns".to_string()],
        }
    }

    /// The same editor on another draft.
    pub fn with_draft(mut self, draft: FeedDraft) -> Self {
        self.draft = draft;
        self
    }

    /// The id of the feed being edited.
    pub fn editing(&self) -> Option<&str> {
        match &self.mode {
            EditorMode::Create => None,
            EditorMode::Edit { id, .. } => Some(id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct FeedsUi {
    pub selected: Option<String>,
    /// `None` is All collections.
    pub collection: Option<String>,
    pub filter: String,
    pub tab: FeedTab,
    pub selected_event: Option<String>,
    /// Delivery attempts of the selected event only.
    pub attempts_for_event: bool,
    pub tail: Tail,
    pub editor: Option<FeedEditor>,
    /// Delete feed is waiting for its confirmation.
    pub confirm_delete: bool,
    /// What the last feed action did, or why it failed.
    pub notice: Option<Notice>,
    /// Bumps after a write, so reads run again.
    pub epoch: u32,
}

/// A line under a panel header that says what an action did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub error: bool,
    pub text: String,
}

impl Notice {
    pub fn ok(text: impl Into<String>) -> Self {
        Self {
            error: false,
            text: text.into(),
        }
    }

    pub fn err(text: impl Into<String>) -> Self {
        Self {
            error: true,
            text: text.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::feeds::{ChangeOp, DeliveryStatus};

    fn ev(n: u32) -> FeedEvent {
        FeedEvent {
            id: format!("e{n}"),
            received_at: String::new(),
            op: ChangeOp::Insert,
            record_id: format!("r{n}"),
            delivery: DeliveryStatus::Acknowledged,
            payload: String::new(),
        }
    }

    fn running() -> Tail {
        let mut t = Tail::default();
        t.start(
            "feed_01",
            FeedSession {
                feed: "feed_01".into(),
                group: "studio_feed_01".into(),
            },
        );
        t
    }

    #[test]
    fn events_arrive_newest_first() {
        let mut t = running();
        t.receive(vec![ev(1), ev(2)]);
        t.receive(vec![ev(3)]);
        let ids: Vec<_> = t.shown.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["e3", "e2", "e1"]);
    }

    #[test]
    fn a_repeated_event_is_not_added_twice() {
        let mut t = running();
        t.receive(vec![ev(1)]);
        t.receive(vec![ev(1)]);
        assert_eq!(t.shown.len(), 1);
    }

    #[test]
    fn a_paused_display_holds_events_until_it_resumes() {
        let mut t = running();
        t.receive(vec![ev(1)]);
        t.paused = true;
        t.receive(vec![ev(2), ev(3)]);
        assert_eq!(t.shown.len(), 1);
        assert_eq!(t.held.len(), 2);
        assert!(t.running(), "pausing the display leaves the session open");
        t.resume();
        let ids: Vec<_> = t.shown.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["e3", "e2", "e1"]);
        assert!(t.held.is_empty());
    }

    #[test]
    fn a_full_held_buffer_reports_what_it_dropped() {
        let mut t = running();
        t.paused = true;
        t.receive((0..(TAIL_LIMIT as u32 + 5)).map(ev).collect());
        assert_eq!(t.held.len(), TAIL_LIMIT);
        assert_eq!(t.skipped, 5);
    }

    #[test]
    fn stopping_ends_the_session_and_keeps_what_was_shown() {
        let mut t = running();
        t.receive(vec![ev(1)]);
        t.paused = true;
        t.receive(vec![ev(2)]);
        t.stop();
        assert!(!t.running());
        assert_eq!(t.shown.len(), 2);
        assert!(t.is_for("feed_01"));
    }

    #[test]
    fn starting_again_clears_the_buffer() {
        let mut t = running();
        t.receive(vec![ev(1)]);
        t.start(
            "feed_02",
            FeedSession {
                feed: "feed_02".into(),
                group: "g".into(),
            },
        );
        assert!(t.shown.is_empty());
        assert!(t.is_for("feed_02"));
    }

    #[test]
    fn the_shown_buffer_is_bounded() {
        let mut t = running();
        t.receive((0..(TAIL_LIMIT as u32 + 20)).map(ev).collect());
        assert_eq!(t.shown.len(), TAIL_LIMIT);
    }
}
