//! The target row: the source or array every panel of the tab works on.

use dioxus::prelude::*;

use crate::models::format::date_minute;
use crate::models::spatial::{GeoSource, RasterSource, SourceRef, SpatialSetup};
use crate::state::spatial::{MapUi, RasterUi, SpatialUi};

/// Choose a source. A map framed on another source frames again.
pub fn choose(ui: SpatialUi, source: SourceRef) {
    let mut map = ui.map;
    let hidden = map.peek().hidden.clone();
    map.set(MapUi {
        hidden,
        ..MapUi::default()
    });
    let mut choice = ui.source;
    choice.set(Some(source));
}

#[component]
pub fn SourceBar(setup: SpatialSetup, source: GeoSource) -> Element {
    let ui = use_context::<SpatialUi>();
    let mut collections: Vec<String> = Vec::new();
    for s in &setup.sources {
        if !collections.contains(&s.source.collection) {
            collections.push(s.source.collection.clone());
        }
    }
    let fields: Vec<String> = setup
        .sources
        .iter()
        .filter(|s| s.source.collection == source.source.collection)
        .map(|s| s.source.field.clone())
        .collect();
    let srid = source.srid.map_or_else(
        || "Source SRID unknown".to_string(),
        |s| format!("Source EPSG:{s}"),
    );
    rsx! {
        div { class: "toolbar nowrap sp-target",
            label { r#for: "sp-layer", class: "toolbar-label", "Active layer" }
            select {
                id: "sp-layer",
                class: "select native",
                onchange: {
                    let setup = setup.clone();
                    move |e| {
                        let name = e.value();
                        if let Some(first) = setup.sources.iter().find(|s| s.source.collection == name) {
                            choose(ui, first.source.clone());
                        }
                    }
                },
                for c in collections.iter() {
                    option { key: "{c}", value: "{c}", selected: *c == source.source.collection, "{c}" }
                }
            }
            label { r#for: "sp-geometry", class: "toolbar-label", "Geometry" }
            select {
                id: "sp-geometry",
                class: "select native",
                onchange: {
                    let collection = source.source.collection.clone();
                    move |e| choose(ui, SourceRef::new(&collection, &e.value()))
                },
                for f in fields.iter() {
                    option { key: "{f}", value: "{f}", selected: *f == source.source.field, "{f}" }
                }
            }
            span { class: "toolbar-note", "{srid}" }
        }
    }
}

#[component]
pub fn RasterBar(setup: SpatialSetup, raster: RasterSource) -> Element {
    let ui = use_context::<SpatialUi>();
    let mut state = ui.raster;
    let index = state.read().slice_index;
    rsx! {
        div { class: "toolbar nowrap sp-target",
            label { r#for: "sp-array", class: "toolbar-label", "Array" }
            select {
                id: "sp-array",
                class: "select native",
                onchange: move |e| {
                    state.set(RasterUi {
                        array: Some(e.value()),
                        ..RasterUi::default()
                    });
                },
                for r in setup.rasters.iter() {
                    option { key: "{r.array}", value: "{r.array}", selected: r.array == raster.array, "{r.array}" }
                }
            }
            span { class: "toolbar-note", "Source EPSG:{raster.srid}" }
            if let Some(axis) = raster.slice_axis.clone() {
                label { r#for: "sp-slice", class: "toolbar-label sp-gap", "Time" }
                select {
                    id: "sp-slice",
                    class: "select native",
                    title: "A coordinate on the array's {axis.name} dimension. It is not AS OF SYSTEM TIME or AS OF VALID TIME.",
                    onchange: move |e| {
                        if let Ok(i) = e.value().parse::<u32>() {
                            state.write().slice_index = i;
                        }
                    },
                    for i in 0..axis.points {
                        option {
                            key: "{i}",
                            value: "{i}",
                            selected: i == index,
                            "{date_minute(axis.at(i), 0)} UTC"
                        }
                    }
                }
            }
        }
    }
}
