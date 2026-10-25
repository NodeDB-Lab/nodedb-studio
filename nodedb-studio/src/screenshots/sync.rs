//! Applies the driver's scene target inside the router.
//!
//! The router and the Explorer selection live inside `Studio`, below where
//! the driver mounts. The driver publishes the target as a root context, and
//! `SceneSync`, mounted in `StudioLayout`, moves the router and the selection
//! to it.

use dioxus::prelude::*;

use crate::routes::Route;
use crate::state::console::ConsoleState;
use crate::state::designer::DesignerState;
use crate::state::events::{EventsSeed, EventsUi};
use crate::state::explorer::{RecordView, Selected};
use crate::state::query::QueryWorkspace;
use crate::state::search::{SearchSeed, SearchUi};
use crate::state::spatial::{SpatialSeed, SpatialUi};
use crate::state::timeseries::{TimeseriesSeed, TimeseriesUi};
use crate::state::vector::{VectorSeed, VectorUi};

/// Where the current studio scene points.
#[derive(Clone, PartialEq)]
pub struct SceneTarget {
    pub route: Route,
    /// The Explorer's selection and view. `None` keeps its own.
    pub explorer: Option<(Selected, RecordView)>,
    /// The Query tabs. `None` keeps the current ones.
    pub query: Option<QueryWorkspace>,
    /// The Console session. `None` keeps the current one.
    pub console: Option<ConsoleState>,
    /// The Designer's selection and drawers. `None` keeps the current ones.
    pub designer: Option<DesignerState>,
    /// The Events & Jobs tabs' state. `None` keeps the current state.
    pub events: Option<EventsSeed>,
    /// The Vector Space drafts and selection. `None` keeps the current ones.
    pub vector: Option<VectorSeed>,
    /// The Timeseries tabs' drafts and open documents. `None` keeps them.
    pub timeseries: Option<TimeseriesSeed>,
    /// The Spatial view, drafts, and selection. `None` keeps the current ones.
    pub spatial: Option<SpatialSeed>,
    /// The Search queries, results, and selections. `None` keeps the current ones.
    pub search: Option<SearchSeed>,
}

/// Renders nothing. Outside a scene run it has no target and does nothing.
#[component]
pub fn SceneSync() -> Element {
    match try_use_context::<Signal<Option<SceneTarget>>>() {
        Some(target) => rsx! { SyncTo { target } },
        None => rsx! {},
    }
}

#[component]
fn SyncTo(target: Signal<Option<SceneTarget>>) -> Element {
    let nav = use_navigator();
    let mut selected = use_context::<Signal<Option<Selected>>>();
    let mut view = use_context::<Signal<RecordView>>();
    let mut query = use_context::<Signal<QueryWorkspace>>();
    let mut console = use_context::<Signal<ConsoleState>>();
    let mut designer = use_context::<Signal<DesignerState>>();
    let events = use_context::<EventsUi>();
    let vector = use_context::<VectorUi>();
    let timeseries = use_context::<TimeseriesUi>();
    let spatial = use_context::<SpatialUi>();
    let search = use_context::<SearchUi>();
    use_effect(move || {
        let Some(t) = target.read().clone() else {
            return;
        };
        if let Some((sel, seeded)) = t.explorer {
            selected.set(Some(sel));
            view.set(seeded);
        }
        if let Some(q) = t.query {
            query.set(q);
        }
        if let Some(c) = t.console {
            console.set(c);
        }
        if let Some(d) = t.designer {
            // The scene staged its changes before this runs. A new epoch
            // makes the page read them.
            let epoch = designer.peek().epoch + 1;
            designer.set(DesignerState { epoch, ..d });
        }
        if let Some(seed) = t.events {
            events.apply(seed);
        }
        if let Some(seed) = t.vector {
            vector.apply(seed, crate::screenshots::vector_scenes::TARGET);
        }
        if let Some(seed) = t.timeseries {
            timeseries.apply(seed);
        }
        if let Some(seed) = t.spatial {
            spatial.apply(seed);
        }
        if let Some(seed) = t.search {
            search.apply(seed);
        }
        nav.replace(t.route);
    });
    rsx! {}
}
