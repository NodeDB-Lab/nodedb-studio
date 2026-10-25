//! The View menu: one checkbox per closable panel of the current page, plus
//! the global Navigation entry and Restore default view.
//!
//! A checked item shows its panel and an unchecked one hides it. Closing a
//! panel with its x unchecks the same item, because both read and write the
//! one `Workspace` signal. The menu never moves panels: placement belongs to
//! panel grips and dividers.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::routes::Route;
use crate::state::workspace::{Surface, Workspace};

/// The View menu of the page the router shows.
#[component]
pub fn ViewMenu() -> Element {
    let route = use_route::<Route>();
    rsx! { ViewMenuFor { surface: route.surface(), navigation: true } }
}

/// The View menu of `surface`. A page outside the router, such as the
/// Connection Manager, passes its own surface. `navigation` adds the sidebar
/// entry, which only a page with a sidebar has.
#[component]
pub fn ViewMenuFor(surface: Surface, navigation: bool) -> Element {
    let mut workspace = use_context::<Signal<Workspace>>();
    let ws = workspace.read();
    let nav_visible = ws.nav_visible();
    let rows: Vec<_> = surface
        .panels()
        .iter()
        .map(|p| (p.id, p.label, p.hint, ws.is_visible(surface, p.id)))
        .collect();
    drop(ws);

    rsx! {
        div {
            class: "popover view-menu",
            role: "menu",
            "aria-label": "View",
            onclick: move |e| e.stop_propagation(),
            div { class: "menu-title", "View" }
            if navigation {
                ViewItem {
                    label: "Navigation",
                    hint: "Show sidebar",
                    checked: nav_visible,
                    on_toggle: move |on: bool| workspace.write().set_nav_visible(on),
                }
            }
            for (id, label, hint, checked) in rows {
                ViewItem {
                    key: "{id}",
                    label,
                    hint,
                    checked,
                    on_toggle: move |on: bool| workspace.write().set_visible(surface, id, on),
                }
            }
            div { class: "menu-sep" }
            button {
                class: "menu-item",
                role: "menuitem",
                onclick: move |_| workspace.write().restore_default(surface),
                Ico { icon: Icon::RotateCcw, size: 16 }
                span { "Restore default view" }
            }
        }
    }
}

#[component]
fn ViewItem(
    label: &'static str,
    hint: &'static str,
    checked: bool,
    on_toggle: EventHandler<bool>,
) -> Element {
    rsx! {
        label { class: "menu-check", role: "menuitemcheckbox", "aria-checked": "{checked}",
            input {
                r#type: "checkbox",
                class: "checkbox",
                checked,
                onchange: move |e| on_toggle.call(e.checked()),
            }
            span { class: "menu-check-text",
                span { class: "menu-check-label", "{label}" }
                span { class: "menu-check-hint", "{hint}" }
            }
        }
    }
}
