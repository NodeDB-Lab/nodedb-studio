//! The Map tab: the map, its layers, and the selected feature.
//!
//! The map shows the result of one bounded read of the current view. Moving
//! the view marks that result stale and starts nothing: Search this area
//! asks again.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side, region_style};
use crate::models::geo_ops::{Bbox, Shape};
use crate::models::geo_proj::map;
use crate::models::spatial::{GeoSource, SourceRef, SpatialSetup, ViewportRequest};
use crate::routes::tabs::SpatialTab;
use crate::services::backend::Backend;
use crate::state::spatial::{MapUi, SpatialUi};
use crate::state::workspace::{Surface, Workspace};
use crate::views::spatial::feature::FeaturePanel;
use crate::views::spatial::layers::LayersPanel;
use crate::views::spatial::map_canvas::MapCanvas;
use crate::views::spatial::map_toolbar::MapToolbar;

const SURFACE: Surface = Surface::Spatial(SpatialTab::Map);
const SIDE_SIZE: PanelSize = PanelSize::width(340.0, 280.0, 520.0);
const LAYERS_SIZE: PanelSize = PanelSize::height(190.0, 110.0, 420.0);

/// The most features one source returns for a view.
pub const RENDER_LIMIT: u32 = 500;
/// Free pixels around a framed box.
const PAD: f64 = 40.0;
/// What an empty source frames.
const WORLD: Bbox = Bbox::new(-180.0, -60.0, 180.0, 75.0);
/// Half the size of the box a feature opens in, in metres.
const OPEN_HALF_M: f64 = 400.0;

/// The sources the map draws now: every listed source whose layer is on.
pub fn visible_sources(setup: &SpatialSetup, m: &MapUi) -> Vec<SourceRef> {
    setup
        .sources
        .iter()
        .filter(|s| m.visible(&s.source.key()))
        .map(|s| s.source.clone())
        .collect()
}

/// Ask for the features inside the current view. A read already running is
/// superseded.
pub fn search_here(m: &mut MapUi, setup: &SpatialSetup) {
    let Some(view) = m.view else { return };
    let sources = visible_sources(setup, m);
    if sources.is_empty() {
        return;
    }
    m.request(ViewportRequest {
        sources,
        bbox: map::bounds(view, m.size),
        limit: RENDER_LIMIT,
    });
}

#[component]
pub fn MapTab(setup: SpatialSetup, source: GeoSource) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let ui = use_context::<SpatialUi>();
    let workspace = use_context::<Signal<Workspace>>();
    let mut map_ui = ui.map;

    // The read being run. A late answer to a read that was superseded no
    // longer matches `pending` and is dropped.
    let pending = use_memo(move || map_ui.read().pending.clone());
    let run = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let _conn = conn();
            let pending = pending();
            async move {
                match pending {
                    Some((seq, req)) => Some((seq, backend.spatial_viewport(&req).await)),
                    None => None,
                }
            }
        })
    };
    use_effect(move || {
        let Some(Some((seq, outcome))) = run.read().clone() else {
            return;
        };
        let mut m = map_ui.write();
        if m.pending.as_ref().map(|p| p.0) != Some(seq) {
            return;
        }
        m.pending = None;
        match outcome {
            Ok(v) => {
                m.shown = Some(v);
                m.error = None;
            }
            // The last result stays on the map, marked.
            Err(e) => m.error = Some(e.to_string()),
        }
    });

    // A feature another tool asked to open: read it, then frame it.
    let opening = use_memo(move || map_ui.read().open.clone());
    let opened = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let _conn = conn();
            let open = opening();
            async move {
                match open {
                    Some(r) => Some((r.clone(), backend.spatial_feature(&r).await)),
                    None => None,
                }
            }
        })
    };
    use_effect(move || {
        let Some(Some((r, outcome))) = opened.read().clone() else {
            return;
        };
        let mut m = map_ui.write();
        if m.open.as_ref() != Some(&r) {
            return;
        }
        m.open = None;
        match outcome {
            Ok(f) => {
                m.frame = match f.shape {
                    Shape::Point(x, y) => Some(Bbox::around(x, y, OPEN_HALF_M)),
                    Shape::Polygon(_) => f.shape.bbox().map(|b| b.grown(0.25)),
                };
            }
            Err(e) => m.error = Some(e.to_string()),
        }
    });

    // Frame the view once the map has a size: on a box something asked for,
    // else on the source's extent.
    use_effect({
        let setup = setup.clone();
        let extent = source.extent;
        move || {
            let (frame, has_view, measured, size) = {
                let m = map_ui.read();
                (m.frame, m.view.is_some(), m.measured, m.size)
            };
            if !measured {
                return;
            }
            let target = match frame {
                Some(b) => Some(b),
                None if !has_view => Some(extent.map_or(WORLD, |e| e.grown(0.12))),
                None => None,
            };
            let Some(bbox) = target else { return };
            let mut m = map_ui.write();
            m.frame = None;
            m.view = Some(map::fit(bbox, size, PAD));
            search_here(&mut m, &setup);
        }
    });

    let (layers_on, feature_on) = {
        let w = workspace.read();
        (
            w.is_visible(SURFACE, "layers"),
            w.is_visible(SURFACE, "feature"),
        )
    };
    let side_style = region_style(&workspace.read(), SURFACE, "side", SIDE_SIZE);

    rsx! {
        div { class: "workbench sp-map-wb",
            Panel { surface: SURFACE, id: "map", title: "Map", class: "fill sp-map-panel",
                MapToolbar {}
                MapCanvas { setup: setup.clone(), source: source.clone() }
            }
            if layers_on || feature_on {
                ResizeHandle { surface: SURFACE, target: "side", size: SIDE_SIZE, side: Side::After }
                div { class: "workbench col sp-side", style: "{side_style}",
                    LayersPanel { setup: setup.clone(), active: source.clone(), size: LAYERS_SIZE }
                    ResizeHandle { surface: SURFACE, target: "layers", size: LAYERS_SIZE, side: Side::Before }
                    FeaturePanel { setup: setup.clone() }
                }
            }
        }
    }
}
