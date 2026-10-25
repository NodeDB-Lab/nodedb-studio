//! Applies the driver's scene target inside the router.
//!
//! The router and the Explorer selection live inside `Studio`, below where
//! the driver mounts. The driver publishes the target as a root context, and
//! `SceneSync`, mounted in `StudioLayout`, moves the router and the selection
//! to it.

use dioxus::prelude::*;

use crate::routes::Route;
use crate::state::explorer::{RecordView, Selected};

/// Where the current studio scene points.
#[derive(Clone, PartialEq)]
pub struct SceneTarget {
    pub route: Route,
    /// The Explorer's selection and view. `None` keeps its own.
    pub explorer: Option<(Selected, RecordView)>,
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
    use_effect(move || {
        let Some(t) = target.read().clone() else {
            return;
        };
        if let Some((sel, seeded)) = t.explorer {
            selected.set(Some(sel));
            view.set(seeded);
        }
        nav.replace(t.route);
    });
    rsx! {}
}
