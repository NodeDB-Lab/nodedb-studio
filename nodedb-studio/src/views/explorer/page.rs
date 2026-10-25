//! The Explorer: Catalog, Records, and Record details.
//!
//! The selection and the way the collection is viewed live above the
//! router, so hiding a panel through the View menu or visiting Data
//! Transfer keeps them. Each collection mounts its own workspace, keyed by
//! name, so a switch starts from that collection's defaults.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::models::collection::{Collection, Profile};
use crate::services::backend::Backend;
use crate::state::explorer::{RecordView, Selected, select_collection};
use crate::state::workspace::Surface;
use crate::views::explorer::array_workspace::ArrayWorkspace;
use crate::views::explorer::catalog::CatalogBody;
use crate::views::explorer::workspace::RecordWorkspace;

pub const SURFACE: Surface = Surface::Explorer;
pub const CATALOG_SIZE: PanelSize = PanelSize::width(252.0, 200.0, 440.0);
pub const DETAILS_SIZE: PanelSize = PanelSize::width(324.0, 280.0, 620.0);

#[component]
pub fn ExplorerPage() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut selected = use_context::<Signal<Option<Selected>>>();
    let mut view = use_context::<Signal<RecordView>>();

    let mut catalog = use_resource(move || {
        let backend = backend.clone();
        // The catalog belongs to the connection: a switch reads the new one.
        let _conn = conn();
        async move { backend.catalog().await }
    });
    let state = resource_state(&catalog);
    let list: Vec<Collection> = state.loaded().cloned().unwrap_or_default();

    // A selection absent from the catalog (first visit, another server, a
    // dropped collection) gives way to the first collection. Never a name
    // the catalog does not hold.
    use_effect(move || {
        let Some(Ok(list)) = catalog.read().clone() else {
            return;
        };
        let present = selected
            .peek()
            .as_ref()
            .is_some_and(|s| list.iter().any(|c| c.name == s.name));
        if present {
            return;
        }
        match list.first() {
            Some(first) => select_collection(&mut selected, &mut view, &first.name),
            None => selected.set(None),
        }
    });

    let current = selected
        .read()
        .as_ref()
        .and_then(|s| list.iter().find(|c| c.name == s.name).cloned());
    let names: Vec<String> = list.iter().map(|c| c.name.clone()).collect();
    let selected_name = current.as_ref().map(|c| c.name.clone());

    rsx! {
        div { class: "page-body",
            div { class: "workbench",
                Panel {
                    surface: SURFACE,
                    id: "catalog",
                    title: "Catalog",
                    size: CATALOG_SIZE,
                    CatalogBody {
                        catalog: state.clone(),
                        selected: selected_name,
                        on_retry: move |_| catalog.restart(),
                    }
                }
                ResizeHandle { surface: SURFACE, target: "catalog", size: CATALOG_SIZE, side: Side::Before }
                match current {
                    Some(c) if c.profile == Profile::Array => rsx! {
                        ArrayWorkspace { key: "{c.name}", collection: c }
                    },
                    Some(c) => rsx! {
                        RecordWorkspace { key: "{c.name}", collection: c, catalog: names }
                    },
                    None => rsx! {
                        EmptyWorkspace {
                            loading: state.is_loading(),
                            error: state.error_message(),
                            empty: state.is_empty(),
                        }
                    },
                }
            }
        }
    }
}

/// The Records and details panels before a collection is chosen, so the
/// View menu still lists both.
#[component]
fn EmptyWorkspace(loading: bool, error: Option<String>, empty: bool) -> Element {
    rsx! {
        Panel { surface: SURFACE, id: "records", title: "Records", class: "fill",
            AsyncView {
                loading,
                empty,
                error,
                empty_message: "This database has no collections to browse.",
            }
            if !loading && !empty {
                div { class: "async-empty", "Select a collection to view its records." }
            }
        }
        ResizeHandle { surface: SURFACE, target: "details", size: DETAILS_SIZE, side: Side::After }
        Panel { surface: SURFACE, id: "details", title: "Record details", size: DETAILS_SIZE,
            div { class: "async-empty", "Select a record to inspect it." }
        }
    }
}
