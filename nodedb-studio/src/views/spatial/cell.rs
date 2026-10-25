//! The Cell panel: the selected cell's stored value and where it is. It is
//! read-only: a cell identity is its coordinates, not a record id.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize};
use crate::components::tabs::TabBar;
use crate::models::format::thousands;
use crate::models::geo_raster::BandMeta;
use crate::models::spatial::{RasterSlice, RasterSource};
use crate::models::spatial_text::{
    crs_unit, factor_text, physical_text, raw_text, trimmed, unit_mark,
};
use crate::routes::Route;
use crate::routes::tabs::SpatialTab;
use crate::state::explorer::{CollectionTab, RecordView, Selected, select_collection};
use crate::state::spatial::{CellTab, SpatialUi};
use crate::state::workspace::Surface;

const SURFACE: Surface = Surface::Spatial(SpatialTab::Raster);

#[component]
fn Rows(rows: Vec<(String, String)>) -> Element {
    rsx! {
        dl { class: "sp-kv",
            for (n , (k , v)) in rows.into_iter().enumerate() {
                div { key: "{n}", class: "sp-kv-row",
                    dt { "{k}" }
                    dd { "{v}" }
                }
            }
        }
    }
}

/// The names of a position's two axes. A geographic CRS reads longitude and
/// latitude, a projected one easting and northing, and an unknown one X and Y.
fn axis_names(srid: u32) -> (&'static str, &'static str) {
    match crs_unit(srid) {
        Some("degrees") => ("Longitude", "Latitude"),
        Some(_) => ("Easting", "Northing"),
        None => ("X", "Y"),
    }
}

fn centre_value(v: f64, srid: u32) -> String {
    match unit_mark(srid) {
        Some("°") => format!("{}°", trimmed(v, 6)),
        Some(m) => {
            let digits = thousands(v.round().abs() as u64);
            format!("{}{digits}{m}", if v < 0.0 { "-" } else { "" })
        }
        None => trimmed(v, 6),
    }
}

fn value_rows(slice: &RasterSlice, meta: &BandMeta, row: u32, col: u32) -> Vec<(String, String)> {
    let raw = slice.raw_at(row, col);
    let raw_shown = raw.map_or_else(|| "Unavailable".to_string(), |r| raw_text(meta, r));
    let physical = match (raw, slice.physical_at(row, col)) {
        (Some(_), Some(p)) => physical_text(p),
        (Some(_), None) => "NoData".to_string(),
        (None, _) => "Unavailable".to_string(),
    };
    vec![
        ("Band".to_string(), meta.name.clone()),
        ("Raw value".to_string(), raw_shown),
        ("Physical value".to_string(), physical),
        (
            "Units".to_string(),
            if meta.units.is_empty() {
                "Unknown".to_string()
            } else {
                meta.units.clone()
            },
        ),
    ]
}

#[component]
pub fn CellPanel(raster: RasterSource, slice: Option<RasterSlice>, size: PanelSize) -> Element {
    let ui = use_context::<SpatialUi>();
    let nav = use_navigator();
    let mut explorer = use_context::<Signal<Option<Selected>>>();
    let mut view = use_context::<Signal<RecordView>>();
    let mut state = ui.raster;
    let (cell, tab) = {
        let s = state.read();
        (s.cell, s.tab)
    };
    let tabs: Vec<&'static str> = CellTab::ALL.iter().map(|t| t.label()).collect();
    let active = CellTab::ALL.iter().position(|t| *t == tab).unwrap_or(0);
    let (xn, yn) = axis_names(raster.srid);

    rsx! {
        Panel { surface: SURFACE, id: "cell", title: "Cell", size,
            match (slice.clone(), cell) {
                (Some(slice), Some((row, col))) if row < slice.rows && col < slice.cols => {
                    let (x, y) = slice.affine.centre(row, col);
                    let meta = slice.meta.clone();
                    rsx! {
                        div { class: "sp-cell",
                            div { class: "sp-cell-pos",
                                span { class: "sp-cell-label", "Selected position" }
                                span { class: "sp-cell-value", "row {row} · col {col}" }
                            }
                            TabBar {
                                class: "sub",
                                items: tabs.clone(),
                                active,
                                on_select: move |i: usize| {
                                    if let Some(t) = CellTab::ALL.get(i) {
                                        state.write().tab = *t;
                                    }
                                },
                            }
                            div { class: "sp-cell-body",
                                match tab {
                                    CellTab::Value => rsx! {
                                        Rows { rows: value_rows(&slice, &meta, row, col) }
                                        h4 { class: "sp-section", "Value metadata" }
                                        Rows {
                                            rows: vec![
                                                (
                                                    "NoData".to_string(),
                                                    meta.nodata.map_or("None declared".to_string(), |n| raw_text(&meta, n)),
                                                ),
                                                (
                                                    "Scale".to_string(),
                                                    meta.scale.map_or("1 (not declared)".to_string(), factor_text),
                                                ),
                                                (
                                                    "Offset".to_string(),
                                                    meta.offset.map_or("0 (not declared)".to_string(), factor_text),
                                                ),
                                            ],
                                        }
                                        h4 { class: "sp-section", "Cell centre" }
                                        Rows {
                                            rows: vec![
                                                (xn.to_string(), centre_value(x, raster.srid)),
                                                (yn.to_string(), centre_value(y, raster.srid)),
                                            ],
                                        }
                                        p { class: "sp-formula", "Physical = raw × scale + offset" }
                                    },
                                    CellTab::Source => rsx! {
                                        Rows {
                                            rows: {
                                                let mut rows = vec![
                                                    ("Array".to_string(), raster.array.clone()),
                                                    ("Band type".to_string(), meta.dtype.clone()),
                                                    ("CRS".to_string(), format!("EPSG:{}", raster.srid)),
                                                    ("GEOREF x".to_string(), raster.x_dim.clone()),
                                                    ("GEOREF y".to_string(), raster.y_dim.clone()),
                                                    (
                                                        "Affine".to_string(),
                                                        format!(
                                                            "[{}]",
                                                            raster.affine.coeffs().iter().map(|c| format!("{c}")).collect::<Vec<_>>().join(", ")
                                                        ),
                                                    ),
                                                ];
                                                for d in raster.dimensions.iter() {
                                                    rows.push((
                                                        format!("Dimension {}", d.name),
                                                        format!(
                                                            "{} to {}, {} steps{}",
                                                            trimmed(d.start, 6),
                                                            trimmed(d.end, 6),
                                                            thousands(u64::from(d.points)),
                                                            if d.unit.is_empty() { String::new() } else { format!(", {}", d.unit) }
                                                        ),
                                                    ));
                                                }
                                                rows
                                            },
                                        }
                                    },
                                }
                            }
                            div { class: "sp-feature-actions",
                                button {
                                    class: "btn",
                                    title: "Open the array's dimensions and attributes in Explorer, read-only",
                                    onclick: {
                                        let array = raster.array.clone();
                                        move |_| {
                                            select_collection(&mut explorer, &mut view, &array);
                                            view.write().tab = CollectionTab::Dimensions;
                                            nav.push(Route::Explorer {});
                                        }
                                    },
                                    Ico { icon: Icon::Database, size: 16 }
                                    "Inspect array"
                                }
                            }
                        }
                    }
                }
                (Some(_), _) => rsx! {
                    div { class: "sp-empty", "Select a cell on the raster to read it." }
                },
                (None, _) => rsx! {
                    div { class: "sp-empty", "The raster has not loaded." }
                },
            }
        }
    }
}
