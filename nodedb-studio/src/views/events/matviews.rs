//! Materialized views: the saved views, the selected view's rows and
//! definition, its refresh policy, and its refresh history.
//!
//! Browsing a view stays in the Explorer and defining one stays in the
//! Designer. This tab shows how a view is kept fresh. A lag the server does
//! not report reads "Not reported", never zero.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side, region_style};
use crate::models::matviews::MaterializedView;
use crate::routes::tabs::EventsTab;
use crate::services::backend::Backend;
use crate::state::events::EventsUi;
use crate::state::events::matviews::PAGE_SIZE;
use crate::state::query::QueryWorkspace;
use crate::state::workspace::{Surface, Workspace};
use crate::views::events::common::NoticeLine;
use crate::views::events::mv_result::ResultRows;
use crate::views::events::mv_side::{DefinitionPanel, HistoryPanel};
use crate::views::events::mv_tasks;

pub const SURFACE: Surface = Surface::Events(EventsTab::MaterializedViews);
const LIST_SIZE: PanelSize = PanelSize::width(300.0, 220.0, 460.0);
const DEFINITION_SIZE: PanelSize = PanelSize::width(380.0, 300.0, 600.0);
const HISTORY_SIZE: PanelSize = PanelSize::height(240.0, 140.0, 460.0);

/// Views whose name contains `filter`, ignoring case.
pub fn matching<'a>(views: &'a [MaterializedView], filter: &str) -> Vec<&'a MaterializedView> {
    let needle = filter.trim().to_lowercase();
    views
        .iter()
        .filter(|v| needle.is_empty() || v.name.to_lowercase().contains(&needle))
        .collect()
}

#[component]
pub fn MatViewsTab() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let nav = use_navigator();
    let mut ui = use_context::<EventsUi>().views;
    let ws = use_context::<Signal<QueryWorkspace>>();
    let layout = use_context::<Signal<Workspace>>();
    let epoch = use_memo(move || ui.read().epoch);

    let mut views = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let (_conn, _epoch) = (conn(), epoch());
            async move { backend.materialized_views().await }
        })
    };
    let state = resource_state(&views);
    let list: Vec<MaterializedView> = state.loaded().cloned().unwrap_or_default();
    let selected_id = use_memo(move || {
        let chosen = ui.read().selected.clone();
        let read = views.read();
        let list = read.as_ref().and_then(|r| r.as_ref().ok())?;
        chosen
            .filter(|id| list.iter().any(|v| &v.id == id))
            .or_else(|| list.first().map(|v| v.id.clone()))
    });
    let selected: Option<MaterializedView> = selected_id
        .read()
        .as_ref()
        .and_then(|id| list.iter().find(|v| &v.id == id).cloned());

    let (filter, notice, page, refreshing) = {
        let u = ui.read();
        (u.filter.clone(), u.notice.clone(), u.page, u.refreshing)
    };
    let rows = matching(&list, &filter);
    let history_style = region_style(&layout.read(), SURFACE, "history", HISTORY_SIZE);

    rsx! {
        div { class: "page-body",
            div { class: "workbench col events-wb",
                div { class: "workbench",
                    Panel {
                        surface: SURFACE,
                        id: "views",
                        title: "Materialized views",
                        size: LIST_SIZE,
                        div { class: "toolbar nowrap",
                            div { class: "search-field grow",
                                Ico { icon: Icon::Search, size: 16 }
                                input {
                                    class: "input",
                                    placeholder: "Filter views…",
                                    "aria-label": "Filter views",
                                    value: "{filter}",
                                    oninput: move |e| ui.write().filter = e.value(),
                                }
                            }
                        }
                        AsyncView {
                            loading: state.is_loading(),
                            empty: state.is_empty(),
                            error: state.error_message(),
                            retriable: state.is_retriable(),
                            on_retry: move |_| views.restart(),
                            empty_message: "This database has no materialized views. Define one in the Designer.",
                        }
                        if state.loaded().is_some() {
                            div { class: "list view-list",
                                for v in rows.iter() {
                                    {
                                        let id = v.id.clone();
                                        let is_selected = selected_id.read().as_deref() == Some(v.id.as_str());
                                        rsx! {
                                            button {
                                                key: "{v.id}",
                                                class: if is_selected { "list-item selected" } else { "list-item" },
                                                "aria-selected": "{is_selected}",
                                                onclick: move |_| {
                                                    let mut w = ui.write();
                                                    w.selected = Some(id.clone());
                                                    w.page = 0;
                                                    w.open_run = None;
                                                },
                                                span { class: "list-item-main", "{v.name}" }
                                            }
                                        }
                                    }
                                }
                                if rows.is_empty() {
                                    div { class: "async-empty", "No view matches the filter." }
                                }
                            }
                        }
                    }
                    ResizeHandle { surface: SURFACE, target: "views", size: LIST_SIZE, side: Side::Before }
                    Panel {
                        surface: SURFACE,
                        id: "result",
                        title: selected.as_ref().map_or_else(|| "Result".to_string(), |v| v.name.clone()),
                        class: "fill",
                        NoticeLine { notice, on_dismiss: move |_| ui.write().notice = None }
                        div { class: "toolbar nowrap view-bar",
                            div { class: "toolbar-spacer" }
                            button {
                                class: "btn primary",
                                disabled: true,
                                title: "Defining a materialized view is a Designer flow that is not designed yet",
                                Ico { icon: Icon::Plus, size: 16 }
                                "New view"
                            }
                            button {
                                class: "btn",
                                disabled: selected.is_none(),
                                title: "Open the definition of the selected view in a new Query tab",
                                onclick: {
                                    let (backend, view) = (backend.clone(), selected.clone());
                                    move |_| {
                                        let Some(view) = view.clone() else { return };
                                        let backend = backend.clone();
                                        spawn_forever(async move { mv_tasks::open_definition(ws, backend, nav, view).await });
                                    }
                                },
                                Ico { icon: Icon::FileCode, size: 16 }
                                "Open definition"
                            }
                            button {
                                class: "btn",
                                disabled: selected.is_none() || refreshing,
                                title: match selected.as_ref() {
                                    Some(v) => format!("Refresh {} now. A manual refresh joins the history.", v.name),
                                    None => "Select a view to refresh".to_string(),
                                },
                                onclick: {
                                    let (backend, view) = (backend.clone(), selected.clone());
                                    move |_| {
                                        let Some(view) = view.clone() else { return };
                                        let backend = backend.clone();
                                        spawn_forever(async move { mv_tasks::refresh(ui, backend, view).await });
                                    }
                                },
                                Ico { icon: Icon::RefreshCw, size: 16 }
                                if refreshing { "Refreshing…" } else { "Refresh" }
                            }
                        }
                        if let Some(v) = selected.clone() {
                            ResultRows { view: v, page, page_size: PAGE_SIZE }
                        } else {
                            div { class: "async-empty", "Select a view to see its rows." }
                        }
                    }
                    ResizeHandle { surface: SURFACE, target: "definition", size: DEFINITION_SIZE, side: Side::After }
                    Panel {
                        surface: SURFACE,
                        id: "definition",
                        title: "Definition",
                        size: DEFINITION_SIZE,
                        if let Some(v) = selected.clone() {
                            DefinitionPanel { view: v }
                        }
                    }
                }
                ResizeHandle { surface: SURFACE, target: "history", size: HISTORY_SIZE, side: Side::After }
                div { class: "workbench", style: "{history_style}",
                    Panel {
                        surface: SURFACE,
                        id: "history",
                        title: "Refresh history",
                        class: "fill",
                        if let Some(v) = selected {
                            HistoryPanel { view: v }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::MockConnectionService;
    use crate::services::matview_data::MatViewData;

    #[tokio::test]
    async fn the_filter_matches_names_ignoring_case() {
        let views = MockConnectionService::ready()
            .materialized_views()
            .await
            .expect("views");
        assert_eq!(matching(&views, "").len(), 5);
        assert_eq!(matching(&views, "SALES").len(), 2);
        assert!(matching(&views, "nope").is_empty());
    }
}
