//! Root component: provides global state and the top-level state machine.
//!
//! Two states, handled by a root conditional (NOT routing — see CLAUDE.md §5):
//!   - Disconnected -> `ConnectionManager` (full screen, no studio chrome)
//!   - Connected    -> `Studio`
//!
//! Global state is exposed as fine-grained signals via context. The backend
//! seam is a `dyn ConnectionService` behind an `Rc`, so swapping the mock for a
//! real NodeDB client later is a one-line change here.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::modals::ModalHost;
use crate::models::notification::Notification;
use crate::services::async_state::AsyncState;
use crate::services::connection_service::{ConnectionService, MockConnectionService};
use crate::state::connection::ActiveConnection;
use crate::state::connections_registry::SavedConnection;
use crate::state::preferences::Preferences;
use crate::state::ui::ModalKind;
use crate::views::connection_manager::ConnectionManager;
use crate::views::studio_shell::Studio;

const STYLES: Asset = asset!("/assets/styles.css");

// Dioxus components are PascalCase by convention; `App` is the root component.
#[allow(non_snake_case)]
pub fn App() -> Element {
    // The single seam to the outside world. Provided as a trait object so a
    // real client can replace the mock without touching consumers.
    let service: Rc<dyn ConnectionService> = Rc::new(MockConnectionService);
    // The real-client stub's instantiability + object-safety behind the seam is
    // proven by `nodedb_service::tests::stub_is_object_safe_behind_rc`, so it is
    // not constructed here on every render.

    use_context_provider(|| service.clone());
    use_context_provider(|| Signal::new(None::<ActiveConnection>));
    let mut registry = use_context_provider(|| Signal::new(Vec::<SavedConnection>::new()));
    // Single source of truth for notifications: the bell badge AND the popover
    // read this one store; user actions (mark-all-read / click) mutate it.
    let mut notifications =
        use_context_provider(|| Signal::new(AsyncState::<Vec<Notification>>::Loading));
    use_context_provider(|| Signal::new(Preferences::default()));
    // Modal state is provided here (not in Studio) because Preferences is
    // reachable while disconnected and via Cmd+, in either state.
    use_context_provider(|| Signal::new(None::<ModalKind>));

    // Seed the registry + notification feed asynchronously, at the seam. The
    // mock resolves instantly; the real client awaits the network. The guard
    // rule: clone the Rc before the async block and `.set()` the signals only
    // AFTER the await resolves — never hold a read/write guard across `.await`.
    // The Resource handle is shared via context so a view (the notification
    // popover's Retry) can reload the feed.
    let reload_feed = {
        let svc = service.clone();
        use_resource(move || {
            let svc = svc.clone();
            async move {
                // Reset to Loading at the start of each (re)load so a Retry shows
                // the spinner. `.set()` holds no guard across the awaits below.
                notifications.set(AsyncState::Loading);
                if let Ok(conns) = svc.list_connections().await {
                    registry.set(conns);
                }
                // Map the seam result straight into the store: Ok(empty)->Empty,
                // Ok(data)->Loaded, Err->Error.
                notifications.set(AsyncState::from_value(Some(svc.notifications().await)));
            }
        })
    };
    use_context_provider(|| reload_feed);

    let active = use_context::<Signal<Option<ActiveConnection>>>();

    rsx! {
        document::Stylesheet { href: STYLES }
        if active.read().is_some() {
            Studio {}
        } else {
            ConnectionManager {}
        }
        // Overlays everything; renders nothing when no modal is open.
        ModalHost {}
    }
}
