//! Studio-internal routing.
//!
//! Routing exists only in the connected state — `App` mounts the `Router`
//! inside `Studio`, never in the Connection Manager. All routes
//! share `StudioLayout`, which renders the persistent chrome (rail, topbar,
//! statusbar) around the content `Outlet`.

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

    /// The topbar breadcrumb for this route, last segment current. Labels
    /// follow the mockup's `updateBreadcrumb`. The database is not part of it;
    /// it lives in the database chip. `collection` is the Explorer's selected
    /// collection, appended when there is one.
    pub fn breadcrumb(&self, collection: Option<&str>) -> Vec<String> {
        let parts: &[&str] = match self {
            Route::Explorer {} => &["Explorer"],
            Route::Query {} => &["Query"],
            Route::Designer {} => &["Designer"],
            Route::GraphExplorer {} => &["Graph Explorer"],
            Route::VectorSpace {} => &["Vector Space"],
            Route::Streams { tab } => match tab.as_str() {
                "landing" => &["Streams & Events"],
                "mv" => &["Streams", "MV inspector"],
                "topics" => &["Streams", "Durable topics"],
                "notify" => &["Streams", "LISTEN/NOTIFY"],
                "cron" => &["Streams", "Scheduled jobs"],
                // "cdc", and any unknown tab, which the Streams view also
                // renders as CDC.
                _ => &["Streams", "CDC consumer"],
            },
            Route::Sync {} => &["Sync"],
            Route::Admin { .. } => &["Admin"],
            Route::Console {} => &["Console"],
            Route::TimeseriesDashboard {} => &["Timeseries dashboard"],
            Route::SpatialView {} => &["Spatial map"],
            Route::FtsInspector {} => &["FTS inspector"],
        };
        let mut crumbs: Vec<String> = parts.iter().map(|p| p.to_string()).collect();
        if let (Route::Explorer {}, Some(name)) = (self, collection) {
            crumbs.push(name.to_string());
        }
        crumbs
    }
}

/// Persistent studio chrome wrapping every route's content.
#[component]
fn StudioLayout() -> Element {
    let nav = use_navigator();
    let route = use_route::<Route>();
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let mut popover = use_context::<Signal<Option<Popover>>>();

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

    rsx! {
        div {
            class: "app",
            // Keys are handled at the `App` root. A click anywhere outside a
            // chip/popover closes the open popover (chips and popover bodies
            // stop propagation).
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

#[cfg(test)]
mod tests {
    use super::*;

    fn crumbs(route: Route, collection: Option<&str>) -> Vec<String> {
        route.breadcrumb(collection)
    }

    #[test]
    fn explorer_appends_the_selected_collection() {
        assert_eq!(
            crumbs(Route::Explorer {}, Some("users")),
            ["Explorer", "users"]
        );
        assert_eq!(crumbs(Route::Explorer {}, None), ["Explorer"]);
    }

    /// The selection belongs to the Explorer; other screens never show it.
    #[test]
    fn other_routes_ignore_the_selection() {
        assert_eq!(crumbs(Route::Query {}, Some("users")), ["Query"]);
    }

    #[test]
    fn streams_tabs_use_the_mockup_labels() {
        let tab = |t: &str| Route::Streams { tab: t.to_string() };
        assert_eq!(crumbs(tab("landing"), None), ["Streams & Events"]);
        assert_eq!(crumbs(tab("cdc"), None), ["Streams", "CDC consumer"]);
        assert_eq!(crumbs(tab("mv"), None), ["Streams", "MV inspector"]);
        assert_eq!(crumbs(tab("cron"), None), ["Streams", "Scheduled jobs"]);
    }

    /// An unknown tab renders as CDC in the Streams view; the breadcrumb must
    /// name the screen actually shown.
    #[test]
    fn unknown_streams_tab_reads_as_cdc() {
        let r = Route::Streams {
            tab: "nope".to_string(),
        };
        assert_eq!(crumbs(r, None), ["Streams", "CDC consumer"]);
    }
}
