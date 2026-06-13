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

use crate::components::modal::ModalHost;
use crate::services::connection_service::{ConnectionService, MockConnectionService};
use crate::state::connection::ActiveConnection;
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

    // Seed the registry and notification feed once, from the service.
    let registry = service.list_connections();
    let notifications = service.notifications();

    use_context_provider(|| service.clone());
    use_context_provider(|| Signal::new(None::<ActiveConnection>));
    use_context_provider(|| Signal::new(registry));
    use_context_provider(|| Signal::new(notifications));
    use_context_provider(|| Signal::new(Preferences::default()));
    // Modal state is provided here (not in Studio) because Preferences is
    // reachable while disconnected and via Cmd+, in either state.
    use_context_provider(|| Signal::new(None::<ModalKind>));

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
