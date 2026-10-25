//! The Query page: Saved queries, the editor above Results, and the Schema
//! (or the selected query's details) on the right.
//!
//! The tabs live above the router in `QueryWorkspace`, so hiding a panel or
//! leaving the page keeps them. The reads here depend on the connection, so
//! a switch reads the new server's schema, library, and transaction.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::resource_state;
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::services::backend::Backend;
use crate::state::query::{QueryWorkspace, TxEpoch};
use crate::state::workspace::{Surface, Workspace};
use crate::views::query::editor::{EDITOR_SIZE, EditorPanel};
use crate::views::query::library::LibraryBody;
use crate::views::query::results::ResultsPanel;
use crate::views::query::schema::SidePanel;

pub const SURFACE: Surface = Surface::Query;
pub const LIBRARY_SIZE: PanelSize = PanelSize::width(252.0, 200.0, 420.0);
pub const SIDE_SIZE: PanelSize = PanelSize::width(324.0, 260.0, 560.0);

#[component]
pub fn QueryPage() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut ws = use_context::<Signal<QueryWorkspace>>();
    let tx_epoch = use_context::<TxEpoch>();
    let layout = use_context::<Signal<Workspace>>();
    let epoch = use_memo(move || ws.read().library_epoch);

    // The tabs the session restores, read once. Later visits keep the
    // user's own.
    let session = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            let _conn = conn();
            async move { backend.query_session().await }
        }
    });
    use_effect(move || {
        let Some(Ok(drafts)) = session.read().clone() else {
            return;
        };
        if !ws.peek().is_loaded() {
            ws.set(QueryWorkspace::from_drafts(&drafts));
        }
    });

    let mut schema = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            let _conn = conn();
            async move { backend.schema_tree().await }
        }
    });
    let mut library = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            let (_conn, _epoch) = (conn(), epoch());
            async move { backend.query_library().await }
        }
    });
    let transaction = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            let (_conn, _epoch) = (conn(), *tx_epoch.0.read());
            async move { backend.transaction().await }
        }
    });
    let databases = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            let _conn = conn();
            async move { backend.databases().await }
        }
    });

    let schema_state = resource_state(&schema);
    let library_state = resource_state(&library);
    let tx = transaction.read().clone().and_then(Result::ok);
    let names = databases
        .read()
        .clone()
        .and_then(Result::ok)
        .unwrap_or_default();
    let results_open = layout.read().is_visible(SURFACE, "results");

    rsx! {
        div { class: "page-body",
            div { class: "workbench query-page",
                Panel {
                    surface: SURFACE,
                    id: "library",
                    title: "Saved queries",
                    size: LIBRARY_SIZE,
                    LibraryBody { library: library_state.clone(), on_retry: move |_| library.restart() }
                }
                ResizeHandle { surface: SURFACE, target: "library", size: LIBRARY_SIZE, side: Side::Before }
                div { class: "workbench col query-main",
                    EditorPanel { databases: names, tx, library: library_state.clone() }
                    if results_open {
                        ResizeHandle { surface: SURFACE, target: "editor", size: EDITOR_SIZE, side: Side::Before }
                    }
                    ResultsPanel {}
                }
                ResizeHandle { surface: SURFACE, target: "schema", size: SIDE_SIZE, side: Side::After }
                SidePanel {
                    schema: schema_state,
                    library: library_state,
                    on_retry: move |_| schema.restart(),
                    size: SIDE_SIZE,
                }
            }
        }
    }
}
