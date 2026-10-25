//! The Raster tab: one band of a georeferenced array, drawn from its raw
//! values, and the cell under inspection.
//!
//! An array is not a table of features. A cell is `(row, col)` plus the
//! array's other coordinates, and the GEOREF alone says where it is in the
//! world. The colors map physical values, so NoData and low valid values
//! never look alike.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::models::format::thousands;
use crate::models::spatial::{RasterRequest, RasterSource};
use crate::models::spatial_text::{trimmed, unit_mark};
use crate::routes::tabs::SpatialTab;
use crate::services::backend::Backend;
use crate::state::spatial::SpatialUi;
use crate::state::workspace::Surface;
use crate::views::spatial::cell::CellPanel;
use crate::views::spatial::raster_canvas::RasterCanvas;
use crate::views::spatial::raster_toolbar::RasterToolbar;

const SURFACE: Surface = Surface::Spatial(SpatialTab::Raster);
const CELL_SIZE: PanelSize = PanelSize::width(340.0, 280.0, 520.0);

/// `0.1° pixels` or `30 m pixels`. A CRS with no known unit gets the number
/// alone, never a guessed unit.
fn pixel_text(raster: &RasterSource) -> String {
    let (sx, sy) = raster.affine.pixel_size();
    let mark = unit_mark(raster.srid);
    let one = |v: f64| match mark {
        Some(m) => format!("{}{m}", trimmed(v, 6)),
        None => trimmed(v, 6),
    };
    let size = if (sx - sy).abs() < 1e-12 * sx.abs().max(1.0) {
        one(sx)
    } else {
        format!("{} × {}", one(sx), one(sy))
    };
    if mark.is_some() {
        format!("{size} pixels")
    } else {
        format!("pixel size {size}, units unknown")
    }
}

#[component]
pub fn RasterTab(raster: RasterSource) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let ui = use_context::<SpatialUi>();
    let rs = ui.raster;

    let (index, band_choice) = {
        let s = rs.read();
        (s.slice_index, s.band.clone())
    };
    let band = band_choice
        .filter(|b| raster.bands.iter().any(|x| x.name == *b))
        .or_else(|| raster.bands.first().map(|b| b.name.clone()))
        .unwrap_or_default();
    let request = RasterRequest {
        array: raster.array.clone(),
        slice_index: index,
        band: band.clone(),
    };
    let mut slice_read = {
        let backend = backend.clone();
        use_resource(use_reactive!(|request| {
            let backend = backend.clone();
            let _conn = conn();
            async move { backend.raster_slice(&request).await }
        }))
    };
    let state = resource_state(&slice_read);
    let slice = state.loaded().cloned();
    let nodata = slice.as_ref().map(|s| s.nodata_cells());

    rsx! {
        div { class: "workbench sp-raster-wb",
            Panel { surface: SURFACE, id: "raster", title: "Raster", class: "fill sp-raster-panel",
                RasterToolbar { raster: raster.clone(), band: band.clone() }
                div { class: "sp-meta",
                    "{thousands(u64::from(raster.rows))} × {thousands(u64::from(raster.cols))} cells · {pixel_text(&raster)}"
                    if let Some(n) = nodata.filter(|n| *n > 0) {
                        " · {thousands(n as u64)} NoData cells"
                    }
                }
                AsyncView {
                    loading: state.is_loading(),
                    empty: false,
                    error: state.error_message(),
                    retriable: state.is_retriable(),
                    on_retry: move |_| slice_read.restart(),
                }
                if let Some(slice) = slice.clone() {
                    RasterCanvas { raster: raster.clone(), slice }
                }
            }
            ResizeHandle { surface: SURFACE, target: "cell", size: CELL_SIZE, side: Side::After }
            CellPanel { raster: raster.clone(), slice, size: CELL_SIZE }
        }
    }
}
