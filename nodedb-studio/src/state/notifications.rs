//! Helpers over the live notification list.
//!
//! The list itself lives in a `Signal<Vec<Notification>>` provided at the app
//! root. These free functions keep the capability-filtering and unread-count
//! logic in one place so the bell badge and the popover list agree.

use crate::models::notification::Notification;
use crate::services::async_state::AsyncState;
use crate::services::error::StudioError;
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

/// Reconcile the local store with the result of a `mark_all_read` write.
///
/// On `Ok` the loaded list is cleared so the badge drops immediately. On `Err`
/// the loaded list is left exactly as it was: the write failed, the read did
/// not, and clearing badges for a write the server rejected is the lie this
/// function exists to prevent. The write error is returned so the caller can
/// show it beside the still-correct list rather than in place of it.
pub fn apply_mark_all_read(
    store: &mut AsyncState<Vec<Notification>>,
    write: Result<(), StudioError>,
) -> Option<StudioError> {
    match write {
        Ok(()) => {
            if let Some(items) = store.loaded_mut() {
                mark_all_read(items);
            }
            None
        }
        Err(e) => Some(e),
    }
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
    fn loaded(items: Vec<Notification>) -> AsyncState<Vec<Notification>> {
        AsyncState::from_value(Some(Ok(items)))
    }

    #[test]
    fn apply_mark_all_read_ok_clears_every_unread() {
        let mut store = loaded(vec![notif("a", true), notif("b", true)]);
        let err = apply_mark_all_read(&mut store, Ok(()));
        assert!(err.is_none());
        assert!(
            store
                .loaded()
                .expect("still loaded")
                .iter()
                .all(|n| !n.unread)
        );
    }

    /// The property the popover used to violate: a failed write must leave
    /// the loaded list untouched, so the badge cannot show "all clear" for
    /// items the server still holds unread.
    #[test]
    fn apply_mark_all_read_err_leaves_list_intact_and_returns_error() {
        let mut store = loaded(vec![notif("a", true), notif("b", false)]);
        let err = apply_mark_all_read(&mut store, Err(StudioError::NotConnected));
        assert!(matches!(err, Some(StudioError::NotConnected)));
        let items = store
            .loaded()
            .expect("a failed write must not discard the loaded list");
        assert!(items[0].unread, "unread flag must survive a failed write");
        assert!(!items[1].unread);
    }
}
