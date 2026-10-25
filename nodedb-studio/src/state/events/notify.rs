//! Notifications page state: Studio's listening session and its display.
//!
//! Clear display and Pause display change this window. Stop listening ends
//! the session. They are separate operations.

use crate::models::listen::{DISPLAY_LIMIT, ListenSession, NotifyMessage};

#[derive(Debug, Clone, PartialEq)]
pub struct NotifyUi {
    pub channel: String,
    pub session: Option<ListenSession>,
    /// Display frozen. Notifications keep arriving into `held`.
    pub paused: bool,
    /// Newest first.
    pub shown: Vec<NotifyMessage>,
    pub held: Vec<NotifyMessage>,
    /// Notifications dropped because the held buffer filled.
    pub skipped: usize,
    pub selected: Option<String>,
    /// The Send notification payload.
    pub send_channel: String,
    pub payload: String,
    pub sending: bool,
    /// What the last send did.
    pub sent: Option<crate::state::events::feeds::Notice>,
    pub error: Option<String>,
}

impl Default for NotifyUi {
    fn default() -> Self {
        Self {
            channel: "user_events".into(),
            session: None,
            paused: false,
            shown: Vec::new(),
            held: Vec::new(),
            skipped: 0,
            selected: None,
            send_channel: "user_events".into(),
            payload: "{\n  \"event\": \"login\",\n  \"user_id\": \"u44182\"\n}".into(),
            sending: false,
            sent: None,
            error: None,
        }
    }
}

impl NotifyUi {
    pub fn listening(&self) -> bool {
        self.session.is_some()
    }

    /// Add notifications a poll delivered, oldest first. A paused display
    /// holds them.
    pub fn receive(&mut self, messages: Vec<NotifyMessage>) {
        for m in messages {
            if self.paused {
                self.held.insert(0, m);
            } else {
                self.shown.insert(0, m);
            }
        }
        if self.held.len() > DISPLAY_LIMIT {
            self.skipped += self.held.len() - DISPLAY_LIMIT;
            self.held.truncate(DISPLAY_LIMIT);
        }
        self.shown.truncate(DISPLAY_LIMIT);
    }

    pub fn resume(&mut self) {
        self.paused = false;
        let held = std::mem::take(&mut self.held);
        for m in held.into_iter().rev() {
            self.shown.insert(0, m);
        }
        self.shown.truncate(DISPLAY_LIMIT);
    }

    /// Empty the display. The session keeps listening.
    pub fn clear(&mut self) {
        self.shown.clear();
        self.held.clear();
        self.skipped = 0;
        self.selected = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(n: u32) -> NotifyMessage {
        NotifyMessage {
            id: format!("m{n}"),
            at: String::new(),
            channel: "user_events".into(),
            payload: "{}".into(),
        }
    }

    fn listening() -> NotifyUi {
        NotifyUi {
            session: Some(ListenSession {
                id: 12,
                channel: "user_events".into(),
            }),
            ..NotifyUi::default()
        }
    }

    #[test]
    fn messages_show_newest_first() {
        let mut ui = listening();
        ui.receive(vec![msg(1), msg(2)]);
        assert_eq!(ui.shown[0].id, "m2");
    }

    #[test]
    fn pausing_holds_and_resuming_shows_without_stopping_the_listener() {
        let mut ui = listening();
        ui.receive(vec![msg(1)]);
        ui.paused = true;
        ui.receive(vec![msg(2), msg(3)]);
        assert_eq!((ui.shown.len(), ui.held.len()), (1, 2));
        assert!(ui.listening());
        ui.resume();
        let ids: Vec<_> = ui.shown.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["m3", "m2", "m1"]);
    }

    #[test]
    fn clearing_the_display_leaves_the_session() {
        let mut ui = listening();
        ui.receive(vec![msg(1)]);
        ui.clear();
        assert!(ui.shown.is_empty());
        assert!(ui.listening());
    }

    #[test]
    fn the_display_is_bounded() {
        let mut ui = listening();
        ui.receive((0..(DISPLAY_LIMIT as u32 + 10)).map(msg).collect());
        assert_eq!(ui.shown.len(), DISPLAY_LIMIT);
    }
}
