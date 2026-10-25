//! Helpers over the live notification list.
//!
//! The list itself lives in a `Signal<AsyncState<Vec<Notification>>>` provided
//! at the app root. These free functions keep the capability filter, the
//! unread count, and the drawer's tab filter in one place, so the bell badge
//! and the drawer list agree.

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

/// Which entries the drawer lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    #[default]
    Unread,
    All,
}

/// The entries a drawer tab lists: visible for the capabilities, and for
/// `Unread` only the unread ones. Order is the feed's.
pub fn listed(items: &[Notification], caps: &Capabilities, filter: Filter) -> Vec<Notification> {
    visible(items, caps)
        .filter(|n| filter == Filter::All || n.unread)
        .cloned()
        .collect()
}

/// Reconcile the local store with the result of a `mark_read` write. On `Err`
/// the entry stays unread, because the server still holds it unread.
pub fn apply_mark_read(
    store: &mut AsyncState<Vec<Notification>>,
    id: &str,
    write: Result<(), StudioError>,
) -> Option<StudioError> {
    match write {
        Ok(()) => {
            if let Some(items) = store.loaded_mut() {
                mark_read(items, id);
            }
            None
        }
        Err(e) => Some(e),
    }
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
    use crate::models::notification::{NotificationKind, NotificationTarget, ScopeKind, Severity};
    use crate::state::connection::Capability;

    fn notif(id: &str, unread: bool) -> Notification {
        Notification {
            id: id.to_string(),
            kind: NotificationKind::Backup,
            severity: Severity::Info,
            required_cap: None,
            title: "t".to_string(),
            body: "b".to_string(),
            scope: "s".to_string(),
            scope_kind: ScopeKind::Database,
            at: 0,
            target: NotificationTarget::AdminAudit,
            subject: None,
            unread,
        }
    }

    fn caps(sync: bool) -> Capabilities {
        Capabilities {
            graph: false,
            vector: false,
            events: false,
            timeseries: false,
            spatial: false,
            search: false,
            sync,
            cluster: false,
            readonly: false,
        }
    }

    #[test]
    fn unread_tab_lists_unread_and_all_tab_lists_everything() {
        let items = vec![notif("a", true), notif("b", false)];
        let ids = |f| {
            listed(&items, &caps(false), f)
                .into_iter()
                .map(|n| n.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(Filter::Unread), ["a"]);
        assert_eq!(ids(Filter::All), ["a", "b"]);
    }

    /// An entry about a feature the connection lacks is in neither tab and
    /// not in the bell count.
    #[test]
    fn capability_gate_hides_entries_from_every_tab_and_the_count() {
        let mut gated = notif("sync", true);
        gated.required_cap = Some(Capability::Sync);
        let items = vec![gated, notif("a", true)];
        assert_eq!(listed(&items, &caps(false), Filter::All).len(), 1);
        assert_eq!(unread_count(&items, &caps(false)), 1);
        assert_eq!(listed(&items, &caps(true), Filter::All).len(), 2);
        assert_eq!(unread_count(&items, &caps(true)), 2);
    }

    #[test]
    fn apply_mark_read_ok_clears_only_that_entry() {
        let mut store = loaded(vec![notif("a", true), notif("b", true)]);
        assert!(apply_mark_read(&mut store, "a", Ok(())).is_none());
        let items = store.loaded().expect("loaded");
        assert!(!items[0].unread);
        assert!(items[1].unread);
    }

    #[test]
    fn apply_mark_read_err_keeps_the_entry_unread() {
        let mut store = loaded(vec![notif("a", true)]);
        let err = apply_mark_read(&mut store, "a", Err(StudioError::NotConnected));
        assert!(err.is_some());
        assert!(store.loaded().expect("loaded")[0].unread);
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
