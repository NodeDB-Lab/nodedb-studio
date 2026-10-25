//! The actions at the right end of the tab strip: Open in Query, and
//! Import raster.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::geo_proj::map;
use crate::models::spatial::SpatialSetup;
use crate::models::spatial_analyze::Availability;
use crate::models::spatial_text::{raster_sql, viewport_sql};
use crate::routes::tabs::SpatialTab;
use crate::services::backend::Backend;
use crate::state::connection::ActiveConnection;
use crate::state::query::QueryWorkspace;
use crate::state::spatial::SpatialUi;
use crate::views::query::tasks::open_in_query;
use crate::views::spatial::map::RENDER_LIMIT;
use crate::views::spatial::page::{resolve_raster, resolve_source};

/// The SQL this tab stands for, with a tab title, or why there is none.
fn query_for(
    tab: SpatialTab,
    setup: &SpatialSetup,
    ui: SpatialUi,
) -> Result<(String, String), String> {
    match tab {
        SpatialTab::Map => {
            let source = resolve_source(setup, ui.source.read().as_ref())
                .ok_or("No geometry source is selected.")?;
            let m = ui.map.read();
            let view = m.view.ok_or("The map has no view yet.")?;
            let sql = viewport_sql(&source, map::bounds(view, m.size), RENDER_LIMIT);
            Ok((format!("{} in view", source.source.collection), sql))
        }
        SpatialTab::Analyze => {
            let source = resolve_source(setup, ui.source.read().as_ref())
                .ok_or("No geometry source is selected.")?;
            let draft = ui.analyze.read().draft(&source.source.key());
            let availability = setup
                .operations
                .iter()
                .find(|o| o.op == draft.form.op)
                .map_or(Availability::Supported, |o| o.availability.clone());
            match draft.form.check(&source, &availability) {
                Ok(req) => Ok((req.title(), req.sql())),
                Err(issues) => Err(issues
                    .first()
                    .map_or("The analysis is not ready.".to_string(), |i| {
                        i.message.clone()
                    })),
            }
        }
        SpatialTab::Raster => {
            let state = ui.raster.read();
            let raster = resolve_raster(setup, state.array.as_deref())
                .ok_or("No georeferenced array is selected.")?;
            let band = state
                .band
                .clone()
                .or_else(|| raster.bands.first().map(|b| b.name.clone()))
                .ok_or("The array has no band.")?;
            let sql = raster_sql(&raster, state.slice_index, &band, state.values);
            Ok((format!("{} {band}", raster.array), sql))
        }
    }
}

#[component]
pub fn TabActions(tab: SpatialTab, setup: Option<SpatialSetup>) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let ui = use_context::<SpatialUi>();
    let ws = use_context::<Signal<QueryWorkspace>>();
    let active = use_context::<Signal<Option<ActiveConnection>>>();
    let nav = use_navigator();

    let database = active
        .read()
        .as_ref()
        .map(|c| c.current_database.clone())
        .unwrap_or_default();
    let query = match &setup {
        Some(s) => query_for(tab, s, ui),
        None => Err("The spatial sources are not loaded.".to_string()),
    };
    let why = query.as_ref().err().cloned();
    rsx! {
        button {
            class: "btn",
            disabled: why.is_some(),
            title: why.unwrap_or_else(|| "Open the equivalent query in a new tab. It does not run.".to_string()),
            onclick: move |_| {
                if let Ok((title, sql)) = query.clone() {
                    spawn_forever(open_in_query(ws, backend.clone(), nav, database.clone(), title, sql));
                }
            },
            Ico { icon: Icon::ExternalLink, size: 16 }
            "Open in Query"
        }
        if tab == SpatialTab::Raster {
            button {
                class: "btn",
                disabled: true,
                title: "Raster import is a staged source-and-metadata workflow. It has no design yet, and selecting a file never runs COPY.",
                Ico { icon: Icon::Upload, size: 16 }
                "Import raster"
            }
        }
    }
}
