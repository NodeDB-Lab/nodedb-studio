//! Applies the driver's scene target inside the router.
//!
//! The router and the Explorer selection live inside `Studio`, below where
//! the driver mounts. The driver publishes the target as a root context, and
//! `SceneSync`, mounted in `StudioLayout`, moves the router and the selection
//! to it.

use dioxus::prelude::*;

use crate::routes::Route;
use crate::state::explorer::Selected;

/// Where the current studio scene points.
#[derive(Clone, PartialEq)]
pub struct SceneTarget {
    pub route: Route,
    /// `None` keeps the Explorer's own selection.
    pub selection: Option<Selected>,
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
    use_effect(move || {
        let Some(t) = target.read().clone() else {
            return;
        };
        if let Some(sel) = t.selection
            && selected.peek().as_ref() != Some(&sel)
        {
            selected.set(Some(sel));
        }
        nav.replace(t.route);
    });
    rsx! {}
}
