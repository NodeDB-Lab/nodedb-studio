//! The persistent studio shell around every route: sidebar, header, page
//! outlet, and footer.

use dioxus::prelude::*;

use crate::components::command_palette::CommandPalette;
use crate::components::footer::Footer;
use crate::components::header::Header;
use crate::components::sidebar::Sidebar;
use crate::routes::Route;
use crate::state::connection::ActiveConnection;
use crate::state::gates;
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
    // never empty. The page is not rendered meanwhile: its first render
    // would start the requests the connection cannot answer.
    let served = {
        let caps = active.read().as_ref().map(|c| c.capabilities);
        gates::serves(caps.as_ref(), route.destination())
    };
    // `served` is plain data, not a signal, so the effect would run once and
    // never again. `use_reactive` re-runs it when the value changes: on a
    // switch to a connection without the screen, and on a move to a screen
    // the connection lacks.
    use_effect(use_reactive((&served,), move |(served,)| {
        if !served {
            nav.replace(Route::Explorer {});
        }
    }));

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
                main { class: "page",
                    if served {
                        Outlet::<Route> {}
                    }
                }
            }
            Footer {}
            CommandPalette {}
        }
    }
}
