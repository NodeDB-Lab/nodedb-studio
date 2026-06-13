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
