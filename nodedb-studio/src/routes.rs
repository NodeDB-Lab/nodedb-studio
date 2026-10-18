//! Studio-internal routing.
//!
//! Routing exists only in the connected state — `App` mounts the `Router`
//! inside `Studio`, never in the Connection Manager. All routes
//! share `StudioLayout`, which renders the persistent chrome (rail, topbar,
//! statusbar) around the content `Outlet`.

use dioxus::html::Key;
use dioxus::prelude::*;

use crate::components::command_palette::CommandPalette;
use crate::components::rail::Rail;
use crate::components::statusbar::Statusbar;
use crate::components::topbar::Topbar;
use crate::state::connection::{ActiveConnection, Capability};
use crate::state::ui::Popover;

use crate::views::admin::Admin;
use crate::views::console::Console;
use crate::views::designer::Designer;
use crate::views::explorer::Explorer;
use crate::views::fts_inspector::FtsInspector;
use crate::views::graph_explorer::GraphExplorer;
use crate::views::query::Query;
use crate::views::spatial_view::SpatialView;
use crate::views::streams::Streams;
use crate::views::sync::Sync;
use crate::views::timeseries_dashboard::TimeseriesDashboard;
use crate::views::vector_space::VectorSpace;

/// Studio views. Streams and Admin carry a sub-tab segment so their lateral
/// nav / sub-tabs are addressable (and deep-linkable) routes.
#[derive(Routable, Clone, PartialEq)]
pub enum Route {
    #[layout(StudioLayout)]
    #[route("/")]
    Explorer {},
    #[route("/query")]
    Query {},
    #[route("/designer")]
    Designer {},
    #[route("/graph")]
    GraphExplorer {},
    #[route("/vector")]
    VectorSpace {},
    #[route("/streams/:tab")]
    Streams { tab: String },
    #[route("/sync")]
    Sync {},
    #[route("/admin/:tab")]
    Admin { tab: String },
    #[route("/console")]
    Console {},
    #[route("/timeseries")]
    TimeseriesDashboard {},
    #[route("/spatial")]
    SpatialView {},
    #[route("/fts")]
    FtsInspector {},
}

impl Route {
    /// Stable key matching the mockup's `data-nav` values. Used for rail
    /// active-state (so any `/streams/*` lights the single "Streams" item).
    pub fn nav_key(&self) -> &'static str {
        match self {
            Route::Explorer {} => "explorer",
            Route::Query {} => "query",
            Route::Designer {} => "designer",
            Route::GraphExplorer {} => "graph",
            Route::VectorSpace {} => "vector",
            Route::Streams { .. } => "streams",
            Route::Sync {} => "sync",
            Route::Admin { .. } => "admin",
            Route::Console {} => "console",
            Route::TimeseriesDashboard {} => "timeseries",
            Route::SpatialView {} => "spatial",
            Route::FtsInspector {} => "fts",
        }
    }

    /// The capability a route requires to be reachable, if any. When the active
    /// connection lacks it, `StudioLayout` redirects to Explorer. Exhaustive on
    /// purpose: a new route must state whether it is gated, or a gated screen
    /// ships reachable on connections that cannot serve it.
    pub fn required_cap(&self) -> Option<Capability> {
        match self {
            Route::GraphExplorer {} => Some(Capability::Graph),
            Route::VectorSpace {} => Some(Capability::Vector),
            Route::Streams { .. } => Some(Capability::Streams),
            Route::TimeseriesDashboard {} => Some(Capability::Timeseries),
            Route::SpatialView {} => Some(Capability::Spatial),
            Route::FtsInspector {} => Some(Capability::Fts),
            Route::Sync {} => Some(Capability::Sync),
            Route::Explorer {}
            | Route::Query {}
            | Route::Designer {}
            | Route::Admin { .. }
            | Route::Console {} => None,
        }
    }
}

/// Persistent studio chrome wrapping every route's content.
#[component]
fn StudioLayout() -> Element {
    let nav = use_navigator();
    let route = use_route::<Route>();
    let mut active = use_context::<Signal<Option<ActiveConnection>>>();
    let mut popover = use_context::<Signal<Option<Popover>>>();
    let mut palette = use_context::<Signal<bool>>();

    // If the current view requires a capability the active connection lacks
    // (e.g. after switching to a connection without Graph), fall back to
    // Explorer so the content area is never empty. Mirrors the mockup's
    // applyCapabilities() fallback.
    use_effect(move || {
        if let Some(cap) = route.required_cap() {
            let has = active
                .read()
                .as_ref()
                .map(|c| c.capabilities.has(cap))
                .unwrap_or(false);
            if !has {
                nav.replace(Route::Explorer {});
            }
        }
    });

    // Studio-only keys: ⌘K and ⌘D need a connection, and the palette and
    // popover signals live in `Studio`. ⌘, and Esc-closes-the-modal are on
    // the `App` root so they also work while disconnected and inside a modal;
    // Esc bubbles there after closing the palette and popover here.
    let on_key = move |e: KeyboardEvent| {
        let meta = e.modifiers().meta() || e.modifiers().ctrl();
        match e.key() {
            Key::Character(c) if meta && c == "k" => {
                e.prevent_default();
                palette.set(true);
            }
            Key::Character(c) if meta && c == "d" => {
                e.prevent_default();
                active.set(None);
            }
            Key::Escape => {
                palette.set(false);
                popover.set(None);
            }
            _ => {}
        }
    };

    rsx! {
        div {
            class: "app",
            tabindex: "0",
            autofocus: true,
            onkeydown: on_key,
            // A click anywhere outside a chip/popover closes the open popover
            // (chips and popover bodies stop propagation).
            onclick: move |_| popover.set(None),
            Rail {}
            div { class: "main",
                Topbar {}
                section { class: "content", Outlet::<Route> {} }
                Statusbar {}
            }
            CommandPalette {}
        }
    }
}
