//! Helpers over the live notification list.
//!
//! The list itself lives in a `Signal<Vec<Notification>>` provided at the app
//! root. These free functions keep the capability-filtering and unread-count
//! logic in one place so the bell badge and the popover list agree.

use crate::models::notification::Notification;
use crate::state::connection::Capabilities;

/// Notifications visible for the given capabilities: an item is hidden when it
/// declares a `required_cap` the connection lacks.
pub fn visible<'a>(
    items: &'a [Notification],
    caps: &Capabilities,
) -> impl Iterator<Item = &'a Notification> {
    let caps = *caps;
    items
        .iter()
        .filter(move |n| n.required_cap.is_none_or(|c| caps.has(c)))
}

/// Count of unread notifications among those visible for the given capabilities.
pub fn unread_count(items: &[Notification], caps: &Capabilities) -> usize {
    visible(items, caps).filter(|n| n.unread).count()
}

/// Clear the unread flag on every notification (the "mark all read" action).
/// Mutates the shared store in place so the bell badge and popover stay in sync.
pub fn mark_all_read(items: &mut [Notification]) {
    for n in items.iter_mut() {
        n.unread = false;
    }
}

/// Clear the unread flag on the single notification with `id`, if present.
pub fn mark_read(items: &mut [Notification], id: &str) {
    if let Some(n) = items.iter_mut().find(|n| n.id == id) {
        n.unread = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::notification::{NotificationTarget, Severity};

    fn notif(id: &str, unread: bool) -> Notification {
        Notification {
            id: id.to_string(),
            severity: Severity::Info,
            group: "g".to_string(),
            required_cap: None,
            title: "t".to_string(),
            desc: "d".to_string(),
            when: "now".to_string(),
            target: NotificationTarget::Query,
            unread,
        }
    }

    #[test]
    fn mark_all_read_clears_every_unread() {
        let mut items = vec![notif("a", true), notif("b", true), notif("c", false)];
        mark_all_read(&mut items);
        assert!(items.iter().all(|n| !n.unread));
    }

    #[test]
    fn mark_read_clears_only_the_match() {
        let mut items = vec![notif("a", true), notif("b", true)];
        mark_read(&mut items, "a");
        assert!(!items[0].unread);
        assert!(items[1].unread);
    }

    #[test]
    fn mark_read_unknown_id_is_noop() {
        let mut items = vec![notif("a", true)];
        mark_read(&mut items, "missing");
        assert!(items[0].unread);
    }
}
