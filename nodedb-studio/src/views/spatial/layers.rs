//! The Layers panel: the sources the map can draw, and the layers made from
//! results and drafts. A layer is a way to draw, not a storage engine.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize};
use crate::models::format::thousands;
use crate::models::spatial::{GeoSource, SpatialSetup};
use crate::routes::tabs::SpatialTab;
use crate::state::spatial::SpatialUi;
use crate::state::workspace::Surface;
use crate::views::spatial::map::search_here;
use crate::views::spatial::target_bar::choose;

const SURFACE: Surface = Surface::Spatial(SpatialTab::Map);

/// The count a source row shows. A count the catalog does not report is
/// Unknown, never 0.
fn count_text(source: &GeoSource) -> String {
    source
        .geometry_count
        .map_or_else(|| "Unknown".to_string(), thousands)
}

#[component]
pub fn LayersPanel(setup: SpatialSetup, active: GeoSource, size: PanelSize) -> Element {
    let ui = use_context::<SpatialUi>();
    let mut map = ui.map;
    let (hidden_result, hidden_draft, result, draft) = {
        let m = map.read();
        (
            !m.visible("result"),
            !m.visible("draft"),
            m.result.clone(),
            m.draft.clone(),
        )
    };
    let many = setup.sources.len() > 1;
    rsx! {
        Panel {
            surface: SURFACE,
            id: "layers",
            title: "Layers",
            size,
            actions: rsx! {
                button {
                    class: "btn sm",
                    disabled: true,
                    title: "A layer from a query or a derived result has no design yet. Choose a source in Active layer.",
                    Ico { icon: Icon::Plus, size: 16 }
                    "Layer"
                }
            },
            div { class: "sp-layers", role: "list",
                for s in setup.sources.iter() {
                    {
                        let key = s.source.key();
                        let on = map.read().visible(&key);
                        let is_active = s.source == active.source;
                        let name = if many { key.clone() } else { s.source.collection.clone() };
                        rsx! {
                            div {
                                key: "{key}",
                                class: if is_active { "sp-layer active" } else { "sp-layer" },
                                role: "listitem",
                                input {
                                    class: "checkbox",
                                    r#type: "checkbox",
                                    "aria-label": "Show {name}",
                                    checked: on,
                                    onchange: {
                                        let (key, setup) = (key.clone(), setup.clone());
                                        move |e| {
                                            let mut m = map.write();
                                            m.set_visible(&key, e.checked());
                                            if e.checked() {
                                                search_here(&mut m, &setup);
                                            }
                                        }
                                    },
                                }
                                span { class: "sp-swatch point", "aria-hidden": "true" }
                                button {
                                    class: "sp-layer-name",
                                    title: "Make {name} the active layer",
                                    "aria-current": "{is_active}",
                                    onclick: {
                                        let source = s.source.clone();
                                        move |_| choose(ui, source.clone())
                                    },
                                    "{name}"
                                }
                                span {
                                    class: "sp-count mono",
                                    title: "Features with a geometry in the source, not in the view",
                                    "{count_text(s)}"
                                }
                            }
                        }
                    }
                }
                if let Some(r) = result {
                    div { class: "sp-layer", role: "listitem",
                        input {
                            class: "checkbox",
                            r#type: "checkbox",
                            "aria-label": "Show the analysis result",
                            checked: !hidden_result,
                            onchange: move |e| map.write().set_visible("result", e.checked()),
                        }
                        span { class: "sp-swatch result", "aria-hidden": "true" }
                        span { class: "sp-layer-name static", title: "{r.title}", "Result: {r.title}" }
                        span { class: "sp-count mono", "{r.count}" }
                        button {
                            class: "icon-btn sm",
                            title: "Remove the result layer. The result stays in Analyze.",
                            "aria-label": "Remove the result layer",
                            onclick: move |_| map.write().result = None,
                            Ico { icon: Icon::X, size: 14 }
                        }
                    }
                }
                if let Some(d) = draft {
                    div { class: "sp-layer", role: "listitem",
                        input {
                            class: "checkbox",
                            r#type: "checkbox",
                            "aria-label": "Show the draft geometry",
                            checked: !hidden_draft,
                            onchange: move |e| map.write().set_visible("draft", e.checked()),
                        }
                        span { class: "sp-swatch draft", "aria-hidden": "true" }
                        span { class: "sp-layer-name static", "Draft: {d.label()}" }
                        button {
                            class: "icon-btn sm",
                            title: "Remove the draft. Nothing was stored.",
                            "aria-label": "Remove the draft",
                            onclick: move |_| map.write().draft = None,
                            Ico { icon: Icon::X, size: 14 }
                        }
                    }
                }
            }
        }
    }
}
