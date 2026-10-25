//! The raster toolbar: how the band is drawn. Rendering choices never change
//! stored values. One row.

use dioxus::prelude::*;

use crate::models::geo_raster::{Palette, RasterCrs, RasterFrame, ValueMode};
use crate::models::spatial::RasterSource;
use crate::state::spatial::SpatialUi;

#[component]
pub fn RasterToolbar(raster: RasterSource, band: String) -> Element {
    let ui = use_context::<SpatialUi>();
    let mut state = ui.raster;
    let (values, palette, crs) = {
        let s = state.read();
        (s.values, s.palette, s.crs)
    };
    let frame = RasterFrame {
        rows: raster.rows,
        cols: raster.cols,
        source: raster.affine,
        srid: raster.srid,
        crs: RasterCrs::Native,
    };
    rsx! {
        div { class: "toolbar nowrap sp-toolbar sp-rtool",
            label { r#for: "sp-mode", class: "toolbar-label", "Mode" }
            select {
                id: "sp-mode",
                class: "select native compact",
                option { value: "single", selected: true, "Single band" }
                option { value: "rgb", disabled: true, title: "RGB needs an explicit mapping of three bands", "RGB" }
            }
            label { r#for: "sp-band", class: "toolbar-label", "Band" }
            select {
                id: "sp-band",
                class: "select native compact",
                onchange: move |e| {
                    let mut s = state.write();
                    s.band = Some(e.value());
                },
                for b in raster.bands.iter() {
                    option { key: "{b.name}", value: "{b.name}", selected: b.name == band, "{b.name}" }
                }
            }
            label { r#for: "sp-values", class: "toolbar-label", "Values" }
            select {
                id: "sp-values",
                class: "select native compact",
                title: "Physical is raw × scale + offset. Raw is what the array stores.",
                onchange: move |e| {
                    if let Some(v) = ValueMode::ALL.iter().find(|v| v.label() == e.value()) {
                        state.write().values = *v;
                    }
                },
                for v in ValueMode::ALL {
                    option { key: "{v.label()}", value: "{v.label()}", selected: v == values, "{v.label()}" }
                }
            }
            label { r#for: "sp-palette", class: "toolbar-label", "Palette" }
            select {
                id: "sp-palette",
                class: "select native compact",
                onchange: move |e| {
                    if let Some(p) = Palette::ALL.iter().find(|p| p.label() == e.value()) {
                        state.write().palette = *p;
                    }
                },
                for p in Palette::ALL {
                    option { key: "{p.label()}", value: "{p.label()}", selected: p == palette, "{p.label()}" }
                }
            }
            span { class: "toolbar-spacer" }
            label { r#for: "sp-rcrs", class: "toolbar-label", "View CRS" }
            select {
                id: "sp-rcrs",
                class: "select native compact",
                title: "Only how the grid is drawn. The array keeps its own CRS.",
                onchange: move |e| {
                    if let Some(c) = RasterCrs::ALL.iter().find(|c| c.label() == e.value()) {
                        let mut s = state.write();
                        s.crs = *c;
                        s.fit = true;
                    }
                },
                for c in RasterCrs::ALL {
                    {
                        let why = frame.supports(c).err();
                        rsx! {
                            option {
                                key: "{c.label()}",
                                value: "{c.label()}",
                                selected: c == crs,
                                disabled: why.is_some(),
                                title: "{why.clone().unwrap_or_default()}",
                                "{c.label()}"
                            }
                        }
                    }
                }
            }
        }
    }
}
