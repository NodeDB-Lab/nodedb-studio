//! The canvas: collection cards the user can drag, the reference connectors
//! between their field rows, and the zoom controls.
//!
//! Where a card sits is the user's layout, kept in the workspace file. It is
//! never a pending change.

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::designer::DesignSchema;
use crate::state::designer::{DesignerState, Target};
use crate::state::workspace::Workspace;
use crate::views::designer::card::Card;
use crate::views::designer::layout::{self, CARD_W, MIN_X, MIN_Y};

/// A card being dragged: its name, the pointer and card position at the
/// start.
#[derive(Clone, PartialEq)]
struct Drag {
    name: String,
    pointer: (f64, f64),
    origin: (f64, f64),
}

#[component]
pub fn Canvas(
    schema: DesignSchema,
    on_add_index: EventHandler<String>,
    on_add_reference: EventHandler<String>,
) -> Element {
    let mut state = use_context::<Signal<DesignerState>>();
    let mut layout_ws = use_context::<Signal<Workspace>>();
    let mut drag = use_signal(|| None::<Drag>);
    let mut view = use_signal(|| None::<std::rc::Rc<MountedData>>);

    let (selected, zoom) = {
        let s = state.read();
        (s.selected.clone(), s.zoom)
    };
    let rects = layout::rects(&schema, &layout_ws.read());
    let (world_w, world_h) = layout::world(&rects);
    let scale = f64::from(zoom) / 100.0;
    let routes: Vec<_> = schema
        .references
        .iter()
        .filter_map(|r| layout::route_of(r, &schema, &rects).map(|route| (r.clone(), route)))
        .collect();

    let fit = move |_| {
        let Some(mounted) = view.peek().clone() else {
            return;
        };
        spawn_forever(async move {
            if let Ok(size) = mounted.get_client_rect().await {
                let mut s = state;
                s.write()
                    .fit((world_w, world_h), (size.width(), size.height()));
            }
        });
    };

    rsx! {
        div { class: "designer-canvas",
            div {
                class: "canvas-scroll",
                onmounted: move |e| view.set(Some(e.data())),
                onclick: move |_| {
                    state.write().selected = None;
                },
                div {
                    class: "canvas-world",
                    style: "width:{world_w}px; height:{world_h}px; zoom:{scale};",
                    svg {
                        class: "canvas-links",
                        width: "{world_w}",
                        height: "{world_h}",
                        "aria-hidden": "true",
                        for (r , route) in routes.iter() {
                            {
                                let on = selected == Some(Target::Reference(r.id.clone()));
                                let (sx, sy) = route.points[0];
                                let (ex, ey) = route.points[route.points.len() - 1];
                                rsx! {
                                    g { key: "{r.id}", class: if on { "link selected" } else { "link" },
                                        path { d: "{route.path()}" }
                                        circle { cx: "{sx}", cy: "{sy}", r: "4" }
                                        circle { cx: "{ex}", cy: "{ey}", r: "4" }
                                    }
                                }
                            }
                        }
                    }
                    for (r , route) in routes.iter() {
                        {
                            let id = r.id.clone();
                            let on = selected == Some(Target::Reference(r.id.clone()));
                            rsx! {
                                button {
                                    key: "{r.id}",
                                    class: if on { "link-label selected" } else { "link-label" },
                                    style: "left:{route.label.0}px; top:{route.label.1}px;",
                                    title: "{r.label()}",
                                    onclick: move |e| {
                                        e.stop_propagation();
                                        state.write().select(Target::Reference(id.clone()));
                                    },
                                    span { class: "link-pill",
                                        "Logical reference"
                                        if r.state.is_draft() {
                                            " · Draft"
                                        }
                                    }
                                    span { class: "link-note", "Not enforced" }
                                }
                            }
                        }
                    }
                    for (c , (_ , rect)) in schema.collections.iter().zip(rects.iter()) {
                        {
                            let name = c.name().to_string();
                            let origin = (rect.x, rect.y);
                            rsx! {
                                div { key: "{c.name()}", onclick: move |e| e.stop_propagation(),
                                    Card {
                                        collection: c.clone(),
                                        x: rect.x,
                                        y: rect.y,
                                        width: CARD_W,
                                        selected: selected.clone(),
                                        on_select: move |t| state.write().select(t),
                                        on_drag: move |pointer| {
                                            // The drag layer takes the mouse-up, so the press
                                            // is the click that selects.
                                            state.write().select(Target::Collection(name.clone()));
                                            drag.set(Some(Drag { name: name.clone(), pointer, origin }));
                                        },
                                        on_add_index: move |n| on_add_index.call(n),
                                        on_add_reference: move |n| on_add_reference.call(n),
                                    }
                                }
                            }
                        }
                    }
                }
                if schema.collections.is_empty() {
                    div { class: "empty-state",
                        Ico { icon: Icon::Database, size: 24 }
                        h2 { "No collections to design" }
                        p { "Create a collection to start a schema." }
                    }
                }
            }
            div { class: "canvas-zoom", role: "group", "aria-label": "Zoom",
                button {
                    class: "icon-btn",
                    "aria-label": "Zoom out",
                    onclick: move |_| state.write().zoom_out(),
                    Ico { icon: Icon::Minus, size: 16 }
                }
                span { class: "zoom-level", "{zoom}%" }
                button {
                    class: "icon-btn",
                    "aria-label": "Zoom in",
                    onclick: move |_| state.write().zoom_in(),
                    Ico { icon: Icon::Plus, size: 16 }
                }
                button { class: "btn ghost", title: "Fit every card in view", onclick: fit, "Fit" }
            }
        }
        if let Some(d) = drag() {
            div {
                class: "drag-capture card-drag",
                onmousemove: move |e| {
                    let p = e.client_coordinates();
                    let x = (d.origin.0 + (p.x - d.pointer.0) / scale).max(MIN_X);
                    let y = (d.origin.1 + (p.y - d.pointer.1) / scale).max(MIN_Y);
                    layout_ws.write().set_card_position(&d.name, x, y);
                },
                onmouseup: move |_| drag.set(None),
                onmouseleave: move |_| drag.set(None),
            }
        }
    }
}
