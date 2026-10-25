//! Change feeds: the list of feeds, the events of the selected feed, and the
//! feed's details. The selection lives in `EventsUi`, above the panels, so
//! hiding a panel through the View menu keeps it.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::models::feeds::ChangeFeed;
use crate::routes::tabs::EventsTab;
use crate::services::backend::Backend;
use crate::state::connection::ActiveConnection;
use crate::state::events::EventsUi;
use crate::state::events::feeds::FeedEditor;
use crate::state::workspace::Surface;
use crate::views::events::common::{NoticeLine, StatusText};
use crate::views::events::feed_details::FeedDetails;
use crate::views::events::feed_editor::FeedSheet;
use crate::views::events::feed_events::EventsPanel;
use crate::views::events::feed_tasks;

pub const SURFACE: Surface = Surface::Events(EventsTab::ChangeFeeds);
const EVENTS_SIZE: PanelSize = PanelSize::height(330.0, 200.0, 560.0);
const DETAILS_SIZE: PanelSize = PanelSize::width(360.0, 300.0, 560.0);

/// Feeds on `collection` (`None` is every collection) whose name, collection,
/// or delivery contains `filter`, ignoring case.
pub fn matching<'a>(
    feeds: &'a [ChangeFeed],
    collection: Option<&str>,
    filter: &str,
) -> Vec<&'a ChangeFeed> {
    let needle = filter.trim().to_lowercase();
    feeds
        .iter()
        .filter(|f| collection.is_none_or(|c| f.collection == c))
        .filter(|f| {
            needle.is_empty()
                || [&f.name, &f.collection, f.delivery.label()]
                    .iter()
                    .any(|s| s.to_lowercase().contains(&needle))
        })
        .collect()
}

/// The collections the filter offers: the catalog, then any collection a feed
/// reads that the catalog does not list.
pub fn collection_options(catalog: &[String], feeds: &[ChangeFeed]) -> Vec<String> {
    let mut out: Vec<String> = catalog.to_vec();
    for f in feeds {
        if !out.contains(&f.collection) {
            out.push(f.collection.clone());
        }
    }
    out.sort();
    out
}

#[component]
pub fn FeedsTab() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let events = use_context::<EventsUi>();
    let mut ui = events.feeds;
    let epoch = use_memo(move || ui.read().epoch);

    let mut feeds = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            // Feeds belong to the connection's database: a switch re-reads them.
            let (_conn, _epoch) = (conn(), epoch());
            async move { backend.change_feeds().await }
        })
    };
    let catalog = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let _conn = conn();
            async move { backend.catalog().await }
        })
    };
    let state = resource_state(&feeds);
    let list: Vec<ChangeFeed> = state.loaded().cloned().unwrap_or_default();

    // The chosen feed while it is still listed, else the first one.
    let selected_id = use_memo(move || {
        let chosen = ui.read().selected.clone();
        let read = feeds.read();
        let list = read.as_ref().and_then(|r| r.as_ref().ok())?;
        chosen
            .filter(|id| list.iter().any(|f| &f.id == id))
            .or_else(|| list.first().map(|f| f.id.clone()))
    });
    let selected: Option<ChangeFeed> = selected_id
        .read()
        .as_ref()
        .and_then(|id| list.iter().find(|f| &f.id == id).cloned());

    let catalog_names: Vec<String> = catalog
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .map(|c| c.iter().map(|x| x.name.clone()).collect())
        .unwrap_or_default();
    let options = collection_options(&catalog_names, &list);
    let database = use_context::<Signal<Option<ActiveConnection>>>()
        .read()
        .as_ref()
        .map(|c| c.current_database.clone())
        .unwrap_or_default();

    let (collection, filter, editor, notice) = {
        let u = ui.read();
        (
            u.collection.clone(),
            u.filter.clone(),
            u.editor.clone(),
            u.notice.clone(),
        )
    };
    let rows = matching(&list, collection.as_deref(), &filter);
    let new_source = {
        let c = collection
            .clone()
            .or_else(|| selected.as_ref().map(|f| f.collection.clone()))
            .or_else(|| catalog_names.first().cloned())
            .unwrap_or_default();
        format!("{database}.{c}")
    };

    rsx! {
        div { class: "page-body",
            div { class: "workbench events-wb",
                div { class: "workbench col",
                    Panel {
                        surface: SURFACE,
                        id: "feeds",
                        title: "Change feeds",
                        class: "fill",
                        div { class: "toolbar nowrap",
                            label { class: "toolbar-select",
                                span { "Collection" }
                                select {
                                    class: "select native",
                                    "aria-label": "Collection",
                                    onchange: move |e| {
                                        let v = e.value();
                                        ui.write().collection = (!v.is_empty()).then_some(v);
                                    },
                                    option { value: "", selected: collection.is_none(), "All collections" }
                                    for c in options.iter() {
                                        option {
                                            key: "{c}",
                                            value: "{c}",
                                            selected: collection.as_deref() == Some(c.as_str()),
                                            "{c}"
                                        }
                                    }
                                }
                            }
                            div { class: "search-field grow",
                                Ico { icon: Icon::Search, size: 16 }
                                input {
                                    class: "input",
                                    placeholder: "Filter feeds…",
                                    "aria-label": "Filter feeds",
                                    value: "{filter}",
                                    oninput: move |e| ui.write().filter = e.value(),
                                }
                            }
                            div { class: "toolbar-spacer" }
                            button { class: "btn", onclick: move |_| feeds.restart(),
                                Ico { icon: Icon::RefreshCw, size: 16 }
                                "Refresh"
                            }
                            button {
                                class: "btn primary",
                                title: "Create a change feed on {new_source}",
                                onclick: move |_| {
                                    ui.write().editor = Some(FeedEditor::create(&new_source));
                                },
                                Ico { icon: Icon::Plus, size: 16 }
                                "Change feed"
                            }
                        }
                        NoticeLine {
                            notice,
                            on_dismiss: move |_| ui.write().notice = None,
                        }
                        AsyncView {
                            loading: state.is_loading(),
                            empty: state.is_empty(),
                            error: state.error_message(),
                            retriable: state.is_retriable(),
                            on_retry: move |_| feeds.restart(),
                            empty_message: "This database has no change feeds. Create one with Change feed.",
                        }
                        if state.loaded().is_some() {
                            div { class: "dt-wrap",
                                table { class: "dt",
                                    thead {
                                        tr {
                                            th { "Name" }
                                            th { "Collection" }
                                            th { "Delivery" }
                                            th { "State" }
                                        }
                                    }
                                    tbody {
                                        for f in rows.iter() {
                                            FeedRow {
                                                key: "{f.id}",
                                                feed: (*f).clone(),
                                                selected: selected_id.read().as_deref() == Some(f.id.as_str()),
                                            }
                                        }
                                    }
                                }
                                if rows.is_empty() {
                                    div { class: "async-empty", "No feed matches the collection and filter." }
                                }
                            }
                        }
                    }
                    ResizeHandle { surface: SURFACE, target: "events", size: EVENTS_SIZE, side: Side::After }
                    Panel {
                        surface: SURFACE,
                        id: "events",
                        title: "Recent events",
                        size: EVENTS_SIZE,
                        EventsPanel { feeds: list.clone(), selected: selected.clone() }
                    }
                }
                ResizeHandle { surface: SURFACE, target: "details", size: DETAILS_SIZE, side: Side::After }
                Panel {
                    surface: SURFACE,
                    id: "details",
                    title: "Feed details",
                    size: DETAILS_SIZE,
                    if let Some(f) = selected {
                        FeedDetails { feed: f, catalog: catalog_names.clone() }
                    } else {
                        div { class: "async-empty", "Select a change feed to see its definition." }
                    }
                }
                if let Some(ed) = editor {
                    FeedSheet { editor: ed, feeds: list.clone(), catalog: catalog_names, database }
                }
            }
        }
    }
}

#[component]
fn FeedRow(feed: ChangeFeed, selected: bool) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let ui = use_context::<EventsUi>().feeds;
    let id = feed.id.clone();
    rsx! {
        tr {
            class: if selected { "clickable selected" } else { "clickable" },
            "aria-selected": "{selected}",
            tabindex: "0",
            onclick: {
                let (backend, id) = (backend.clone(), id.clone());
                move |_| {
                    let (backend, id) = (backend.clone(), id.clone());
                    spawn_forever(async move { feed_tasks::select(ui, backend, id).await });
                }
            },
            onkeydown: move |e| {
                if e.key() == Key::Enter {
                    let (backend, id) = (backend.clone(), id.clone());
                    spawn_forever(async move { feed_tasks::select(ui, backend, id).await });
                }
            },
            td { class: "key", "{feed.name}" }
            td { "{feed.collection}" }
            td { "{feed.delivery.label()}" }
            td { StatusText { tone: feed.state.tone(), text: feed.state.label().to_string() } }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::MockConnectionService;
    use crate::services::feed_data::FeedData;

    #[tokio::test]
    async fn the_collection_filter_and_text_filter_combine() {
        let feeds = MockConnectionService::ready()
            .change_feeds()
            .await
            .expect("feeds");
        assert_eq!(matching(&feeds, None, "").len(), 3);
        assert_eq!(matching(&feeds, Some("sales"), "").len(), 2);
        assert_eq!(matching(&feeds, Some("customers"), "").len(), 1);
        assert_eq!(matching(&feeds, None, "WEBHOOK").len(), 2);
        assert_eq!(matching(&feeds, Some("sales"), "audit").len(), 1);
        assert!(matching(&feeds, Some("sales"), "customer").is_empty());
    }

    #[tokio::test]
    async fn a_feed_collection_missing_from_the_catalog_is_still_offered() {
        let feeds = MockConnectionService::ready()
            .change_feeds()
            .await
            .expect("feeds");
        let catalog = vec!["sales".to_string(), "users".to_string()];
        let options = collection_options(&catalog, &feeds);
        assert_eq!(options, ["customers", "sales", "users"]);
    }
}
