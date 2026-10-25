//! Spatial: the page tabs, the target row, and the active tab.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::components::tabs::{RouteTabs, tab_items};
use crate::models::spatial::{GeoSource, RasterSource, SourceRef, SpatialSetup};
use crate::routes::Route;
use crate::routes::tabs::SpatialTab;
use crate::services::backend::Backend;
use crate::state::spatial::SpatialUi;
use crate::views::spatial::actions::TabActions;
use crate::views::spatial::analyze::AnalyzeTab;
use crate::views::spatial::map::MapTab;
use crate::views::spatial::raster::RasterTab;
use crate::views::spatial::target_bar::{RasterBar, SourceBar};

/// The source the user chose while it is listed, else the first one.
pub fn resolve_source(setup: &SpatialSetup, choice: Option<&SourceRef>) -> Option<GeoSource> {
    choice
        .and_then(|c| setup.source(c))
        .or_else(|| setup.sources.first())
        .cloned()
}

/// The array the user chose while it is listed, else the first one.
pub fn resolve_raster(setup: &SpatialSetup, choice: Option<&str>) -> Option<RasterSource> {
    choice
        .and_then(|c| setup.raster(c))
        .or_else(|| setup.rasters.first())
        .cloned()
}

#[component]
pub fn SpatialPage(tab: SpatialTab) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let ui = use_context::<SpatialUi>();

    let mut setup_read = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let _conn = conn();
            async move { backend.spatial_setup().await }
        })
    };
    let state = resource_state(&setup_read);
    let setup: Option<SpatialSetup> = state.loaded().cloned();
    let choice = ui.source.read().clone();
    let raster_choice = ui.raster.read().array.clone();
    let source = setup
        .as_ref()
        .and_then(|s| resolve_source(s, choice.as_ref()));
    let raster = setup
        .as_ref()
        .and_then(|s| resolve_raster(s, raster_choice.as_deref()));

    rsx! {
        RouteTabs {
            items: tab_items(SpatialTab::ALL, |t| t.label(), |tab| Route::Spatial { tab }),
            current: Route::Spatial { tab },
            actions: rsx! { TabActions { tab, setup: setup.clone() } },
        }
        div { class: "page-body sp-page",
            AsyncView {
                loading: state.is_loading(),
                empty: false,
                error: state.error_message(),
                retriable: state.is_retriable(),
                on_retry: move |_| setup_read.restart(),
            }
            if state.is_empty() {
                div { class: "empty-state",
                    Ico { icon: Icon::Map, size: 24 }
                    h2 { "No spatial data" }
                    p {
                        "This database has no collection with a geometry field and no georeferenced array. Add a geometry field in the Designer, and it appears here."
                    }
                }
            }
            if let Some(setup) = setup.clone() {
                match tab {
                    SpatialTab::Map | SpatialTab::Analyze => rsx! {
                        if let Some(source) = source {
                            SourceBar { setup: setup.clone(), source: source.clone() }
                            if tab == SpatialTab::Map {
                                MapTab { setup: setup.clone(), source }
                            } else {
                                AnalyzeTab { setup: setup.clone(), source }
                            }
                        } else {
                            div { class: "empty-state",
                                Ico { icon: Icon::MapPin, size: 24 }
                                h2 { "No geometry source" }
                                p {
                                    "No collection on this connection has a geometry field. Georeferenced arrays are on the Raster tab."
                                }
                            }
                        }
                    },
                    SpatialTab::Raster => rsx! {
                        if let Some(raster) = raster {
                            RasterBar { setup: setup.clone(), raster: raster.clone() }
                            RasterTab { raster }
                        } else {
                            div { class: "empty-state",
                                Ico { icon: Icon::Grid3x3, size: 24 }
                                h2 { "No georeferenced array" }
                                p {
                                    "No array on this connection has a GEOREF. An array without one still reads in Explorer, but it has no place on a map."
                                }
                            }
                        }
                    },
                }
            }
        }
    }
}
