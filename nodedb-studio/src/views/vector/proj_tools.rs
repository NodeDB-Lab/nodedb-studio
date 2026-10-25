//! Controls over the map: zoom, tools, the legend, and Go to record.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::vector_projection::{Category, Projection};
use crate::state::vector::{
    Drafts, MapTool, MapView, ProjectionUi, Selection, VectorUi, ZOOM_STEP,
};

fn edit(mut drafts: Signal<Drafts>, key: &str, f: impl FnOnce(&mut ProjectionUi)) {
    f(drafts.write().projection_mut(key));
}

#[component]
pub fn MapTools(tkey: String, view: MapView, tool: MapTool) -> Element {
    let key = tkey;
    let ui = use_context::<VectorUi>();
    let drafts = ui.drafts;
    let pressed = |t: MapTool| tool == t;
    rsx! {
        div { class: "pj-tools", role: "toolbar", "aria-label": "Map tools",
            div { class: "pj-zoom",
                button {
                    class: "btn icon-only",
                    title: "Zoom out",
                    "aria-label": "Zoom out",
                    onclick: {
                        let key = key.clone();
                        move |_| edit(drafts, &key, |st| st.view = st.view.zoomed(1.0 / ZOOM_STEP))
                    },
                    Ico { icon: Icon::Minus, size: 16 }
                }
                span { class: "zoom-level", "{view.percent_label()}%" }
                button {
                    class: "btn icon-only",
                    title: "Zoom in",
                    "aria-label": "Zoom in",
                    onclick: {
                        let key = key.clone();
                        move |_| edit(drafts, &key, |st| st.view = st.view.zoomed(ZOOM_STEP))
                    },
                    Ico { icon: Icon::Plus, size: 16 }
                }
            }
            button {
                class: "btn",
                title: "Fit the whole projection",
                onclick: {
                    let key = key.clone();
                    move |_| edit(drafts, &key, |st| st.view = MapView::default())
                },
                Ico { icon: Icon::Maximize, size: 16 }
                "Fit"
            }
            button {
                class: if pressed(MapTool::Pan) { "btn icon-only pressed" } else { "btn icon-only" },
                title: "Pan: click the map to center it there",
                "aria-label": "Pan tool",
                "aria-pressed": "{pressed(MapTool::Pan)}",
                onclick: {
                    let key = key.clone();
                    move |_| edit(drafts, &key, |st| st.tool = MapTool::Pan)
                },
                Ico { icon: Icon::Move, size: 16 }
            }
            button {
                class: if pressed(MapTool::Select) { "btn icon-only pressed" } else { "btn icon-only" },
                title: "Select: click a point to read its record",
                "aria-label": "Select tool",
                "aria-pressed": "{pressed(MapTool::Select)}",
                onclick: {
                    let key = key.clone();
                    move |_| edit(drafts, &key, |st| st.tool = MapTool::Select)
                },
                Ico { icon: Icon::MousePointer2, size: 16 }
            }
        }
    }
}

/// The color legend. Each entry names its value and count, so color is
/// never the only cue. Pressing one emphasizes that group.
#[component]
pub fn Legend(
    tkey: String,
    field: String,
    categories: Vec<Category>,
    focus: Option<u8>,
) -> Element {
    let key = tkey;
    let ui = use_context::<VectorUi>();
    let drafts = ui.drafts;
    rsx! {
        div { class: "pj-legend", role: "group", "aria-label": "Colored by {field}",
            span { class: "pj-legend-title", "{field}" }
            for (i , c) in categories.iter().enumerate() {
                {
                    let on = focus == Some(i as u8);
                    let key = key.clone();
                    rsx! {
                        button {
                            key: "{c.label}",
                            class: if on { "pj-legend-item on" } else { "pj-legend-item" },
                            "aria-pressed": "{on}",
                            title: "Emphasize {c.label}",
                            onclick: move |_| edit(drafts, &key, |st| {
                                st.focus = if on { None } else { Some(i as u8) };
                            }),
                            span { class: "pj-swatch c{i % 6}" }
                            "{c.label}"
                            span { class: "pj-legend-count", "{c.count}" }
                        }
                    }
                }
            }
        }
    }
}

/// Type a record id to find it in the sample. The keyboard path to a point.
#[component]
pub fn GoTo(tkey: String, projection: std::rc::Rc<Projection>, note: Option<String>) -> Element {
    let key = tkey;
    let ui = use_context::<VectorUi>();
    let drafts = ui.drafts;
    let mut selected = ui.selected;
    let text = drafts.read().projection(&key).goto;
    rsx! {
        form {
            class: "pj-goto",
            onsubmit: {
                let key = key.clone();
                let projection = projection.clone();
                move |e| {
                    e.prevent_default();
                    let id = drafts.peek().projection(&key).goto.trim().to_string();
                    if id.is_empty() {
                        return;
                    }
                    match projection.point(&id) {
                        Some(p) => {
                            let (x, y) = (p.x, -p.y);
                            selected.set(Some(Selection { target: key.clone(), id }));
                            edit(drafts, &key, |st| {
                                st.view = st.view.centered(x, y);
                                st.goto_note = None;
                            });
                        }
                        None => edit(drafts, &key, |st| {
                            st.goto_note = Some(format!("{id} is not in this sample."));
                        }),
                    }
                }
            },
            label { class: "sr-only", r#for: "pj-goto", "Go to record" }
            input {
                id: "pj-goto",
                class: "input mono",
                placeholder: "Go to record, e.g. art103",
                value: "{text}",
                oninput: {
                    let key = key.clone();
                    move |e| {
                        let v = e.value();
                        edit(drafts, &key, |st| {
                            st.goto.clone_from(&v);
                            st.goto_note = None;
                        });
                    }
                },
            }
            if let Some(n) = note {
                span { class: "pj-goto-note", role: "status", "{n}" }
            }
        }
    }
}
