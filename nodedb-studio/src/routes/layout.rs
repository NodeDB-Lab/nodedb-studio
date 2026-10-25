//! The persistent studio shell around every route: sidebar, header, page
//! outlet, and footer.

use dioxus::prelude::*;

use crate::components::command_palette::CommandPalette;
use crate::components::footer::Footer;
use crate::components::header::Header;
use crate::components::sidebar::Sidebar;
use crate::routes::Route;
use crate::state::connection::ActiveConnection;
use crate::state::ui::Popover;
use crate::state::workspace::Workspace;

#[component]
pub fn StudioLayout() -> Element {
    let nav = use_navigator();
    let route = use_route::<Route>();
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let mut popover = use_context::<Signal<Option<Popover>>>();
    let workspace = use_context::<Signal<Workspace>>();

    // A route the active connection cannot serve (after a switch to a
    // connection without Graph, say) falls back to Explorer, so the page is
    // never empty.
    use_effect(move || {
        if let Some(cap) = route.destination().required_cap() {
            let has = active
                .read()
                .as_ref()
                .is_some_and(|c| c.capabilities.has(cap));
            if !has {
                nav.replace(Route::Explorer {});
            }
        }
    });

    // Moves the router to the screenshot scene's target, when that feature is
    // built.
    #[cfg(all(feature = "screenshots", target_os = "linux"))]
    let scene_sync = rsx! { crate::screenshots::SceneSync {} };
    #[cfg(not(all(feature = "screenshots", target_os = "linux")))]
    let scene_sync = rsx! {};

    let ws = workspace.read();
    let sidebar_state = if !ws.nav_visible() {
        "hidden"
    } else if ws.nav_collapsed() {
        "collapsed"
    } else {
        "expanded"
    };

    rsx! {
        {scene_sync}
        div {
            class: "shell",
            "data-sidebar": sidebar_state,
            // Keys are handled at the `App` root. A click outside a trigger
            // or popover closes the open popover: triggers and popover bodies
            // stop propagation.
            onclick: move |_| popover.set(None),
            Sidebar {}
            div { class: "shell-main",
                Header {}
                main { class: "page", Outlet::<Route> {} }
            }
            Footer {}
            CommandPalette {}
        }
    }
}
