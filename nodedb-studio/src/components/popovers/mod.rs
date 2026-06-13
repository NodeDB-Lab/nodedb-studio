//! Topbar popovers. Each renders only when it is the open popover (the shared
//! `Signal<Option<Popover>>`). They are nested inside their trigger chip so the
//! ported CSS positions them correctly, and stop click propagation so a click
//! inside doesn't bubble to the shell's close-on-click handler.

pub mod avatar_popover;
pub mod connection_popover;
pub mod database_popover;
pub mod notification_popover;
