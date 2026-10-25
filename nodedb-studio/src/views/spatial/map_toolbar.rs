//! The map toolbar: the pointer tools, the display CRS, and the basemap. One
//! row. Below a narrow width only the active tool keeps its name.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::state::spatial::{Draft, MapTool, SpatialUi};

fn icon(tool: MapTool) -> Icon {
    match tool {
        MapTool::Select => Icon::MousePointer2,
        MapTool::Pan => Icon::Move,
        MapTool::Rectangle => Icon::Square,
        MapTool::Polygon => Icon::Hexagon,
        MapTool::Measure => Icon::Ruler,
    }
}

#[component]
pub fn MapToolbar() -> Element {
    let ui = use_context::<SpatialUi>();
    let mut map = ui.map;
    let tool = map.read().tool;
    rsx! {
        div { class: "toolbar nowrap sp-toolbar", role: "toolbar", "aria-label": "Map tools",
            for t in MapTool::ALL {
                button {
                    key: "{t.label()}",
                    class: if t == tool { "btn sm sp-tool active" } else { "btn sm sp-tool" },
                    "aria-pressed": "{t == tool}",
                    "aria-label": "{t.label()}",
                    title: "{t.hint()}",
                    onclick: move |_| {
                        let mut m = map.write();
                        // An unfinished polygon belongs to its tool.
                        if t != MapTool::Polygon
                            && matches!(m.draft, Some(Draft::Polygon { done: false, .. }))
                        {
                            m.draft = None;
                        }
                        if t != MapTool::Measure {
                            m.measure.clear();
                        }
                        m.tool = t;
                    },
                    Ico { icon: icon(t), size: 16 }
                    span { class: if t == tool { "tb-label keep" } else { "tb-label" }, "{t.label()}" }
                }
            }
            span { class: "toolbar-spacer" }
            label { r#for: "sp-crs", class: "toolbar-label", "View CRS" }
            select {
                id: "sp-crs",
                class: "select native compact",
                title: "The map draws in Web Mercator. Stored coordinates keep their own CRS.",
                option { value: "3857", selected: true, "EPSG:3857" }
            }
            label { r#for: "sp-basemap", class: "toolbar-label", "Basemap" }
            select {
                id: "sp-basemap",
                class: "select native compact",
                title: "No tile provider is set up, so the map draws only your geometry.",
                option { value: "none", selected: true, "None" }
                option { value: "tiles", disabled: true, "Tiles: no provider set up" }
            }
        }
    }
}
