//! LISTEN/NOTIFY fixtures and the in-memory listener the mock backend runs.
//!
//! A session sees only what is sent after it starts listening. The mock also
//! plays a slow ambient stream of `user_events` so a session has something
//! to show.

use std::collections::VecDeque;

use crate::data::mock::events::topics::NOW;
use crate::models::format::date_second;
use crate::models::json_text::json_issue;
use crate::models::listen::{ListenSession, NotifyChannel, NotifyMessage};
use crate::services::error::StudioError;

/// The ten notifications the design shows on `user_events`, newest first.
#[cfg(any(test, all(feature = "screenshots", target_os = "linux")))]
const SEEN: [(&str, &str); 10] = [
    (
        "2026-06-13 13:42:18",
        "{\"event\":\"login\",\"user_id\":\"u44182\"}",
    ),
    (
        "2026-06-13 13:42:02",
        "{\"event\":\"page_view\",\"user_id\":\"u44182\",\"page\":\"/dash\"}",
    ),
    (
        "2026-06-13 13:41:37",
        "{\"event\":\"logout\",\"user_id\":\"u99831\"}",
    ),
    (
        "2026-06-13 13:41:12",
        "{\"event\":\"login\",\"user_id\":\"u77620\"}",
    ),
    (
        "2026-06-13 13:40:58",
        "{\"event\":\"signup\",\"user_id\":\"u55491\",\"plan\":\"pro\"}",
    ),
    (
        "2026-06-13 13:40:21",
        "{\"event\":\"login\",\"user_id\":\"u33105\"}",
    ),
    (
        "2026-06-13 13:39:56",
        "{\"event\":\"page_view\",\"user_id\":\"u33105\",\"page\":\"/docs\"}",
    ),
    (
        "2026-06-13 13:39:20",
        "{\"event\":\"login\",\"user_id\":\"u22417\"}",
    ),
    (
        "2026-06-13 13:38:11",
        "{\"event\":\"logout\",\"user_id\":\"u77620\"}",
    ),
    (
        "2026-06-13 13:37:48",
        "{\"event\":\"page_view\",\"user_id\":\"u44182\",\"page\":\"/pricing\"}",
    ),
];

/// The first session id: the design's listener is session 12.
const FIRST_SESSION: u32 = 12;

/// What the ambient stream sends next, in turn.
const AMBIENT: [&str; 4] = [
    "{\"event\":\"login\",\"user_id\":\"u61002\"}",
    "{\"event\":\"page_view\",\"user_id\":\"u61002\",\"page\":\"/billing\"}",
    "{\"event\":\"signup\",\"user_id\":\"u61077\",\"plan\":\"team\"}",
    "{\"event\":\"logout\",\"user_id\":\"u61002\"}",
];

/// The notifications of the design, newest first.
#[cfg(any(test, all(feature = "screenshots", target_os = "linux")))]
pub fn seen_messages() -> Vec<NotifyMessage> {
    SEEN.iter()
        .enumerate()
        .map(|(i, (at, payload))| NotifyMessage {
            id: format!("{FIRST_SESSION}-{}", SEEN.len() - i),
            at: (*at).to_string(),
            channel: "user_events".to_string(),
            payload: (*payload).to_string(),
        })
        .collect()
}

pub fn seeded_channels() -> Vec<NotifyChannel> {
    [
        "user_events",
        "order_updates",
        "cache_invalidate",
        "deploy_events",
    ]
    .iter()
    .map(|name| NotifyChannel {
        name: (*name).to_string(),
        // The server does not report how many sessions listen.
        listeners: None,
    })
    .collect()
}

#[derive(Debug)]
struct Live {
    session: ListenSession,
    queue: VecDeque<NotifyMessage>,
    seq: u32,
}

#[derive(Debug)]
pub struct ListenStore {
    next_session: u32,
    live: Vec<Live>,
    tick: i64,
    ambient: usize,
}

impl ListenStore {
    pub fn seeded() -> Self {
        Self {
            next_session: FIRST_SESSION,
            live: Vec::new(),
            tick: 0,
            ambient: 0,
        }
    }

    pub fn channels(&self) -> Vec<NotifyChannel> {
        seeded_channels()
    }

    pub fn listen(&mut self, channel: &str) -> Result<ListenSession, StudioError> {
        if !self.channels().iter().any(|c| c.name == channel) {
            return Err(StudioError::UnknownObject {
                kind: "channel",
                id: channel.to_string(),
            });
        }
        let session = ListenSession {
            id: self.next_session,
            channel: channel.to_string(),
        };
        self.next_session += 1;
        self.live.push(Live {
            session: session.clone(),
            queue: VecDeque::new(),
            seq: 0,
        });
        Ok(session)
    }

    pub fn unlisten(&mut self, session: &ListenSession) {
        self.live.retain(|l| l.session.id != session.id);
    }

    fn stamp(&mut self) -> String {
        self.tick += 7;
        date_second(NOW + self.tick, 0)
    }

    /// Everything that arrived for `session` since the last poll. A quiet
    /// poll on `user_events` brings the next ambient notification.
    pub fn poll(&mut self, session: &ListenSession) -> Result<Vec<NotifyMessage>, StudioError> {
        let at = self.stamp();
        let ambient = AMBIENT[self.ambient % AMBIENT.len()];
        let Some(live) = self.live.iter_mut().find(|l| l.session.id == session.id) else {
            return Err(StudioError::UnknownObject {
                kind: "listening session",
                id: session.id.to_string(),
            });
        };
        let mut out: Vec<NotifyMessage> = live.queue.drain(..).collect();
        if out.is_empty() && live.session.channel == "user_events" {
            live.seq += 1;
            out.push(NotifyMessage {
                id: format!("{}-{}", live.session.id, 1000 + live.seq),
                at,
                channel: live.session.channel.clone(),
                payload: ambient.to_string(),
            });
            self.ambient += 1;
        }
        Ok(out)
    }

    /// Deliver to every session listening on `channel`. A notification sent
    /// to a channel nobody listens on is dropped, as on the server.
    pub fn send(&mut self, channel: &str, payload: &str) -> Result<usize, StudioError> {
        if !self.channels().iter().any(|c| c.name == channel) {
            return Err(StudioError::UnknownObject {
                kind: "channel",
                id: channel.to_string(),
            });
        }
        if let Some(reason) = json_issue(payload) {
            return Err(StudioError::Rejected { reason });
        }
        let at = self.stamp();
        let mut delivered = 0;
        for live in self
            .live
            .iter_mut()
            .filter(|l| l.session.channel == channel)
        {
            live.seq += 1;
            live.queue.push_back(NotifyMessage {
                id: format!("{}-{}", live.session.id, 2000 + live.seq),
                at: at.clone(),
                channel: channel.to_string(),
                payload: payload.split_whitespace().collect::<Vec<_>>().join(""),
            });
            delivered += 1;
        }
        Ok(delivered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_design_messages_are_newest_first_with_unique_ids() {
        let m = seen_messages();
        assert_eq!(m.len(), 10);
        assert_eq!(m[0].at, "2026-06-13 13:42:18");
        assert_eq!(m[9].at, "2026-06-13 13:37:48");
        let mut ids: Vec<_> = m.iter().map(|x| x.id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 10);
    }

    #[test]
    fn the_first_session_is_twelve_and_ids_do_not_repeat() {
        let mut s = ListenStore::seeded();
        let a = s.listen("user_events").expect("listens");
        s.unlisten(&a);
        let b = s.listen("user_events").expect("listens");
        assert_eq!((a.id, b.id), (12, 13));
        assert!(s.listen("nope").is_err());
    }

    #[test]
    fn a_sent_notification_reaches_only_listeners_of_its_channel() {
        let mut s = ListenStore::seeded();
        let on_orders = s.listen("order_updates").expect("listens");
        let on_users = s.listen("user_events").expect("listens");
        assert_eq!(s.send("order_updates", "{\"id\": 7}").expect("sends"), 1);
        let got = s.poll(&on_orders).expect("polls");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].payload, "{\"id\":7}");
        // The user_events listener only sees the ambient stream.
        let users = s.poll(&on_users).expect("polls");
        assert_eq!(users.len(), 1);
        assert!(users[0].payload.contains("\"event\""));
        // Nothing queued and no ambient stream on this channel.
        assert!(s.poll(&on_orders).expect("polls").is_empty());
    }

    #[test]
    fn nothing_is_replayed_to_a_new_session() {
        let mut s = ListenStore::seeded();
        s.send("order_updates", "{\"id\": 1}").expect("sends");
        let late = s.listen("order_updates").expect("listens");
        assert!(s.poll(&late).expect("polls").is_empty());
    }

    #[test]
    fn a_stopped_session_cannot_poll_and_a_bad_payload_is_refused() {
        let mut s = ListenStore::seeded();
        let a = s.listen("user_events").expect("listens");
        assert!(s.send("user_events", "{ nope").is_err());
        s.unlisten(&a);
        assert!(s.poll(&a).is_err());
    }
}
