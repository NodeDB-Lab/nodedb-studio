//! Header popovers and the notification drawer. Each renders only when it is
//! the open popover (the shared `Signal<Option<Popover>>`). They are nested
//! inside their trigger's anchor so the CSS positions them, and stop click
//! propagation so a click inside does not reach the shell's close-on-click
//! handler.

pub mod avatar_popover;
pub mod connection_popover;
pub mod database_popover;
pub mod notification_drawer;
