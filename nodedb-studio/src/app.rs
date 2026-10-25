//! Root component: provides global state and the top-level state machine.
//!
//! Two states, handled by a root conditional rather than by routing, because
//! the Connection Manager is not a route: the Router mounts inside `Studio`.
//!   - Disconnected -> `ConnectionManager` (full screen, no studio chrome)
//!   - Connected    -> `Studio`, with the manager over it on demand
//!
//! Global state is exposed as fine-grained signals via context. The backend
//! seam is a `dyn Backend` behind an `Rc`, so swapping the mock for a
//! real NodeDB client later is a one-line change here.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::AsyncView;
use crate::modals::ModalHost;
use crate::models::notification::Notification;
use crate::services::MockConnectionService;
use crate::services::async_state::AsyncState;
use crate::services::backend::Backend;
use crate::state::admin::AdminUi;
use crate::state::cluster::ClusterUi;
use crate::state::connection::{ActiveConnection, ConnectError};
use crate::state::connection_form::ConnectionForm;
use crate::state::connections_registry::SavedConnection;
use crate::state::console::ConsoleState;
use crate::state::designer::DesignerState;
use crate::state::events::EventsUi;
use crate::state::explorer::{RecordView, RecordsEpoch, TransferNotice};
use crate::state::graph::GraphUi;
use crate::state::panel_registry::DynamicPanels;
use crate::state::persist::{PREFERENCES, PersistMode, Store, WORKSPACE};
use crate::state::preferences::Preferences;
use crate::state::query::{QueryWorkspace, TxEpoch};
use crate::state::record_editor::{DeleteRequest, RecordEditor};
use crate::state::search::SearchUi;
use crate::state::spatial::SpatialUi;
use crate::state::sync::SyncUi;
use crate::state::timeseries::TimeseriesUi;
use crate::state::ui::{
    ManagerOpen, ManagerSelection, ModalKind, PaletteQuery, Popover, PrefsPane,
};
use crate::state::vector::VectorUi;
use crate::state::workspace::Workspace;
use crate::views::connections::ConnectionManager;
use crate::views::studio_shell::Studio;

// Order matters: later sheets override earlier ones.
const TOKENS: Asset = asset!("/assets/tokens.css");
const BASE: Asset = asset!("/assets/base.css");
const CONTROLS: Asset = asset!("/assets/controls.css");
const SHELL: Asset = asset!("/assets/shell.css");
const PANELS: Asset = asset!("/assets/panels.css");
const CONNECTIONS: Asset = asset!("/assets/connections.css");
const OVERLAYS: Asset = asset!("/assets/overlays.css");
const EXPLORER: Asset = asset!("/assets/explorer.css");
const WORKBENCH: Asset = asset!("/assets/workbench.css");
const DESIGNER: Asset = asset!("/assets/designer.css");
const EVENTS: Asset = asset!("/assets/events.css");
const GRAPH: Asset = asset!("/assets/graph.css");
const VECTOR: Asset = asset!("/assets/vector.css");
const TIMESERIES: Asset = asset!("/assets/timeseries.css");
const SPATIAL: Asset = asset!("/assets/spatial.css");
const SEARCH: Asset = asset!("/assets/search.css");
const CLUSTER: Asset = asset!("/assets/cluster.css");
const SYNC: Asset = asset!("/assets/sync.css");
const ADMIN: Asset = asset!("/assets/admin.css");

/// Quiet period before a changed preference or layout is written to disk.
const SAVE_DELAY: std::time::Duration = std::time::Duration::from_millis(400);

/// Writes `value` to `store` once it has stopped changing for `SAVE_DELAY`.
/// The first run only records the loaded value: nothing changed yet.
fn use_debounced_save<T>(value: Signal<T>, store: Store, mode: PersistMode)
where
    T: serde::Serialize + 'static,
{
    let mut generation = use_signal(|| 0u64);
    use_effect(move || {
        // Reading subscribes the effect to every change of `value`.
        let _changed = value.read();
        let mine = *generation.peek() + 1;
        generation.set(mine);
        if mine == 1 {
            return;
        }
        spawn(async move {
            tokio::time::sleep(SAVE_DELAY).await;
            if *generation.peek() == mine {
                store.save(mode, &*value.peek());
            }
        });
    });
}

// Dioxus components are PascalCase by convention; `App` is the root component.
#[allow(non_snake_case)]
pub fn App() -> Element {
    // The single seam to the outside world. Provided as a trait object so a
    // real client can replace the mock without touching consumers. Built once:
    // a resource re-runs with the closure from the latest render, so an
    // instance per render answers those re-runs from fresh state (a
    // connection switch undid mark-all-read).
    let service: Rc<dyn Backend> =
        use_context_provider(|| Rc::new(MockConnectionService::ready()) as Rc<dyn Backend>);
    // The real-client stub's instantiability + object-safety behind the seam is
    // proven by `nodedb::service::tests::stub_is_object_safe_behind_rc`, so it is
    // not constructed here.
    let mut active = use_context_provider(|| Signal::new(None::<ActiveConnection>));
    // The active connection's name. Per-connection reads subscribe to this,
    // not to `active`: a database pick or a failed switch also writes
    // `active`, and neither changes what the connection's server answers.
    let conn_name = use_memo(move || active.read().as_ref().map(|c| c.name.clone()));
    use_context_provider(|| conn_name);
    let mut registry = use_context_provider(|| Signal::new(Vec::<SavedConnection>::new()));
    // Single source of truth for notifications: the bell badge AND the popover
    // read this one store; user actions (mark-all-read / click) mutate it.
    let mut notifications =
        use_context_provider(|| Signal::new(AsyncState::<Vec<Notification>>::Loading));
    // Personal state from disk: preferences and workspace layout. Each change
    // is written back after a short quiet period, so a panel drag writes once
    // when it settles, not on every pointer move.
    let persist = try_use_context::<PersistMode>().unwrap_or(PersistMode::On);
    let prefs = use_context_provider(|| Signal::new(PREFERENCES.load::<Preferences>(persist)));
    let workspace = use_context_provider(|| Signal::new(WORKSPACE.load::<Workspace>(persist)));
    use_debounced_save(prefs, PREFERENCES, persist);
    use_debounced_save(workspace, WORKSPACE, persist);
    // Modal state is provided here (not in Studio) because Preferences is
    // reachable while disconnected and via Cmd+, in either state.
    let mut modal = use_context_provider(|| Signal::new(None::<ModalKind>));
    // The open header popover and the command palette. Studio-only in use, but
    // provided here so the root key handler below can open and close them.
    let mut popover = use_context_provider(|| Signal::new(None::<Popover>));
    let mut palette = use_context_provider(|| Signal::new(false));
    // The palette's search text, above the palette so a scene can type into it.
    use_context_provider(|| PaletteQuery(Signal::new(String::new())));
    // The connection dialog's fields. The opener fills them, and the dialog
    // clears them (the secret included) when it closes.
    use_context_provider(|| Signal::new(ConnectionForm::blank()));
    // The record dialogs read what their opener put here. The Explorer's
    // view of its collection and the write counter sit above both the studio
    // and the modal host, which are siblings.
    use_context_provider(|| Signal::new(None::<RecordEditor>));
    use_context_provider(|| Signal::new(None::<DeleteRequest>));
    use_context_provider(|| RecordsEpoch(Signal::new(0)));
    use_context_provider(|| TransferNotice(Signal::new(None)));
    use_context_provider(|| Signal::new(RecordView::default()));
    // The Query tabs and the Console transcript outlive their pages, and the
    // sidebar badge counts the tabs. The transaction counter wakes every
    // reader of the session's transaction after a change.
    use_context_provider(|| Signal::new(QueryWorkspace::default()));
    use_context_provider(|| Signal::new(ConsoleState::default()));
    // The Designer's selection and open drawers outlive the page. Staged
    // changes live behind the seam.
    use_context_provider(|| Signal::new(DesignerState::default()));
    // Events & Jobs keeps each tab's selection, filter, tail, and open sheet
    // while the user is on another page.
    EventsUi::provide();
    // Graph Explorer keeps each mode's query, form, selection, and canvas.
    GraphUi::provide();
    // Vector Space keeps each tab's drafts, results, and the selected record.
    VectorUi::provide();
    // Timeseries keeps each tab's draft, result, open dashboard, and changes
    // waiting for review. A page lists the panels it creates at run time here,
    // for the View menu.
    TimeseriesUi::provide();
    // Spatial keeps the map view and selection, each source's analysis draft,
    // and the raster cell while the user is on another page.
    SpatialUi::provide();
    // Search keeps each tab's query, result, and selection.
    SearchUi::provide();
    // Cluster keeps each tab's selection, filters, and open review.
    ClusterUi::provide();
    // Sync keeps each tab's selection, filters, and open sheet.
    SyncUi::provide();
    // Administration keeps each tab's selection, drafts, and open review.
    AdminUi::provide();
    use_context_provider(|| DynamicPanels(Signal::new(Vec::new())));
    use_context_provider(|| TxEpoch(Signal::new(0)));
    // Whether the manager shows over an open session, and which connection it
    // highlights.
    let mut manager = use_context_provider(|| Signal::new(ManagerOpen::default()));
    use_context_provider(|| Signal::new(ManagerSelection::default()));
    // Connect failures are provided here, not inside the views that start them:
    // the command palette and the switch popover both close on click, so an
    // error they owned would be dropped before it could render.
    let connect_error = use_context_provider(|| Signal::new(ConnectError(None)));

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
            // The feed belongs to the connection. Reading its name here
            // subscribes the resource, so connecting, switching or
            // disconnecting re-reads it instead of leaving the previous
            // connection's bell up.
            let _conn = conn_name();
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

    // Every app key, in one handler on the root. A key reaches a handler only
    // from a focused element inside it, and a click focuses the nearest
    // focusable ancestor: this root for a click in a modal, which renders
    // outside the studio's `.app`. Keys split across the two went dead
    // whenever focus sat on the wrong one.
    let on_key = move |e: KeyboardEvent| {
        // A key pressed while an IME composes text belongs to the IME: Esc
        // there cancels the candidate, not the modal around the input.
        if e.is_composing() {
            return;
        }
        let meta = e.modifiers().meta() || e.modifiers().ctrl();
        // Keys that act on a session need one, and need the studio in view.
        let connected = active.peek().is_some() && !manager.peek().0;
        match e.key() {
            Key::Character(c) if meta && c == "k" && connected => {
                e.prevent_default();
                palette.set(true);
            }
            Key::Character(c) if meta && c == "d" && connected => {
                e.prevent_default();
                active.set(None);
            }
            // Not over an open modal: replacing it discards what was typed there.
            Key::Character(c) if meta && c == "," && modal.peek().is_none() => {
                e.prevent_default();
                modal.set(Some(ModalKind::Preferences(PrefsPane::Appearance)));
            }
            Key::Escape => {
                palette.set(false);
                popover.set(None);
                modal.set(None);
            }
            _ => {}
        }
    };
    // Unmounting the focused element drops focus to `<body>`, outside this
    // root, and keys go dead until the next click. Two things unmount it:
    // closing the modal or palette (Esc from one of their inputs), and
    // disconnecting (the studio goes). After either, focus moves back here.
    let mut root = use_signal(|| None::<Rc<MountedData>>);
    let focus_root = move || {
        if let Some(el) = root.peek().clone() {
            spawn(async move {
                // Focus is best-effort: if it fails, a click anywhere in the
                // window focuses this root again.
                let _ = el.set_focus(true).await;
            });
        }
    };
    // Also runs once `root` mounts, which gives the app its first focus.
    use_effect(move || {
        let _mounted = root.read().is_some();
        if modal.read().is_none() && !palette() {
            focus_root();
        }
    });
    use_effect(move || {
        if conn_name.read().is_none() {
            // A reconnect must not reopen these.
            palette.set(false);
            popover.set(None);
            manager.set(ManagerOpen::default());
            focus_root();
        }
    });
    // Rendered through the same component every failed read uses, so the markup
    // and styling live in one place. It is fixed-position and above the modal
    // scrim: `html, body` are `100vh; overflow: hidden`, so a banner in normal
    // flow would push the footer off-screen, and a failed "Save & connect"
    // has to be readable without closing the form that caused it.
    // `retriable` is deliberately false: the way
    // to retry a connect is the Connect button the user just pressed, which is
    // still on screen, and a second affordance here would need the name and
    // credentials of the attempt that failed. The message clears when the next
    // attempt starts (see the call sites) or succeeds.
    let connect_error_msg = connect_error.read().0.as_ref().map(|e| e.to_string());
    let attrs = prefs.read().root_attrs();
    let managing = manager.read().0;

    // The screenshot scene runner, when that feature is built. It renders
    // first so the scene target it provides exists before the studio mounts.
    #[cfg(all(feature = "screenshots", target_os = "linux"))]
    let scene_driver = rsx! { crate::screenshots::ScreenshotDriver {} };
    #[cfg(not(all(feature = "screenshots", target_os = "linux")))]
    let scene_driver = rsx! {};

    rsx! {
        document::Stylesheet { href: TOKENS }
        document::Stylesheet { href: BASE }
        document::Stylesheet { href: CONTROLS }
        document::Stylesheet { href: SHELL }
        document::Stylesheet { href: PANELS }
        document::Stylesheet { href: CONNECTIONS }
        document::Stylesheet { href: OVERLAYS }
        document::Stylesheet { href: EXPLORER }
        document::Stylesheet { href: WORKBENCH }
        document::Stylesheet { href: DESIGNER }
        document::Stylesheet { href: EVENTS }
        document::Stylesheet { href: GRAPH }
        document::Stylesheet { href: VECTOR }
        document::Stylesheet { href: TIMESERIES }
        document::Stylesheet { href: SPATIAL }
        document::Stylesheet { href: SEARCH }
        document::Stylesheet { href: CLUSTER }
        document::Stylesheet { href: SYNC }
        document::Stylesheet { href: ADMIN }
        {scene_driver}
        div {
            class: "studio-root",
            "data-theme": attrs.theme,
            "data-density": attrs.density,
            "data-font-size": attrs.font_size,
            "data-editor-font": attrs.editor_font,
            "data-motion": attrs.motion,
            tabindex: "-1",
            onkeydown: on_key,
            onmounted: move |e| root.set(Some(e.data())),
        if let Some(msg) = connect_error_msg {
            div { class: "connect-error-bar",
                AsyncView { loading: false, empty: false, error: Some(msg) }
            }
        }
        // The studio stays mounted behind the manager, so a visit to Manage
        // connections keeps its tabs and selections.
        if active.read().is_some() {
            div { class: "studio-host", hidden: managing, Studio {} }
        }
        if active.read().is_none() || managing {
            ConnectionManager {}
        }
        // Overlays everything; renders nothing when no modal is open.
        ModalHost {}
        }
    }
}
