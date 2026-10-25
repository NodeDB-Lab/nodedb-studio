//! The Indexes tab: the text and vector indexes of a collection, their build
//! history, and the selected index's definition, coverage, and activity.
//!
//! Studio reads and shows. Definitions change in the Designer, and a rebuild
//! is a reviewed plan before it is a job.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::models::search::SearchSetup;
use crate::models::search_index::{IndexOverview, RunHistory};
use crate::routes::tabs::SearchTab;
use crate::services::backend::Backend;
use crate::state::search::{IndexesUi, SearchUi};
use crate::state::workspace::Surface;
use crate::views::search::index_details::DetailsBody;
use crate::views::search::index_history::HistoryBody;
use crate::views::search::index_list::IndexListBody;
use crate::views::search::page::resolve;
use crate::views::search::rebuild_sheet::RebuildSheet;
use crate::views::search::run_sheet::RunSheet;

const SURFACE: Surface = Surface::Search(SearchTab::Indexes);
const LIST_SIZE: PanelSize = PanelSize::height(352.0, 200.0, 560.0);
const DETAILS_SIZE: PanelSize = PanelSize::width(480.0, 360.0, 640.0);

pub fn edit(mut i: Signal<IndexesUi>, f: impl FnOnce(&mut IndexesUi)) {
    f(&mut i.write());
}

#[component]
pub fn IndexesTab(setup: SearchSetup) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let ui = use_context::<SearchUi>();
    let i = ui.indexes;

    let choice = i.read().collection.clone();
    let collection = resolve(&setup, choice.as_deref()).map(|c| c.name.clone());
    let epoch = i.read().epoch;

    let mut overview_read = {
        let backend = backend.clone();
        use_resource(use_reactive!(|collection, epoch| {
            let backend = backend.clone();
            let _conn = conn();
            let _epoch = epoch;
            async move {
                match collection {
                    Some(c) => backend.search_indexes(&c).await,
                    None => Ok(IndexOverview {
                        collection: String::new(),
                        indexes: Vec::new(),
                    }),
                }
            }
        }))
    };
    let state = resource_state(&overview_read);
    let overview: Option<IndexOverview> = state.loaded().cloned();

    let ui_state = i.read().clone();
    let names: Vec<String> = overview
        .as_ref()
        .map(|o| o.indexes.iter().map(|x| x.name.clone()).collect())
        .unwrap_or_default();
    let selected = ui_state
        .selected
        .clone()
        .filter(|s| names.contains(s))
        .or_else(|| names.first().cloned());
    let history_index = ui_state
        .history_index
        .clone()
        .filter(|s| names.contains(s))
        .or_else(|| selected.clone());

    let history_read = {
        let backend = backend.clone();
        use_resource(use_reactive!(|history_index, epoch| {
            let backend = backend.clone();
            let _conn = conn();
            let _epoch = epoch;
            async move {
                match history_index {
                    Some(name) => backend.search_build_history(&name).await,
                    None => Ok(RunHistory {
                        index: String::new(),
                        runs: Vec::new(),
                    }),
                }
            }
        }))
    };
    let history_state = resource_state(&history_read);
    let history: Option<RunHistory> = history_state.loaded().cloned();

    let collection_name = collection.clone().unwrap_or_default();
    let names_of_collections: Vec<String> =
        setup.collections.iter().map(|c| c.name.clone()).collect();

    rsx! {
        div { class: "workbench srch-indexes",
            div { class: "workbench col srch-ix-left",
                Panel { surface: SURFACE, id: "indexes", title: "Search indexes", size: LIST_SIZE,
                    IndexListBody {
                        collections: names_of_collections,
                        collection: collection_name.clone(),
                        filter: ui_state.filter.clone(),
                        indexes: overview.as_ref().map(|o| o.indexes.clone()).unwrap_or_default(),
                        selected: selected.clone(),
                    }
                    AsyncView {
                        loading: state.is_loading(),
                        empty: state.is_empty(),
                        error: state.error_message(),
                        retriable: state.is_retriable(),
                        on_retry: move |_| overview_read.restart(),
                    }
                }
                ResizeHandle { surface: SURFACE, target: "indexes", size: LIST_SIZE, side: Side::Before }
                Panel { surface: SURFACE, id: "history", title: "Build history", class: "fill",
                    HistoryBody {
                        names: names.clone(),
                        index: history_index.clone(),
                        history,
                        loading: history_state.is_loading(),
                        error: history_state.error_message(),
                        all_runs: ui_state.all_runs,
                    }
                }
            }
            ResizeHandle { surface: SURFACE, target: "details", size: DETAILS_SIZE, side: Side::After }
            Panel { surface: SURFACE, id: "details", title: "Index details", size: DETAILS_SIZE,
                DetailsBody {
                    index: overview
                        .as_ref()
                        .and_then(|o| selected.as_deref().and_then(|s| o.index(s)))
                        .cloned(),
                    tab: ui_state.tab,
                }
            }
        }
        if let Some(id) = ui_state.run.clone() {
            RunSheet { id }
        }
        if let Some(index) = ui_state.plan.clone() {
            RebuildSheet { index, error: ui_state.plan_error.clone() }
        }
    }
}
