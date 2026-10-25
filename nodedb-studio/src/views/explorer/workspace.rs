//! The Records and Record details panels of a collection with records.
//!
//! The reads live here, above both panels: the schema and the session's
//! rights once per collection, the page of records for the current view,
//! and the selected record fresh from the server. A write bumps the epoch,
//! which re-reads the page and the record.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, ResizeHandle, Side};
use crate::components::tabs::TabBar;
use crate::models::collection::{Collection, Profile};
use crate::models::format::{clock, offset_label};
use crate::models::graph_records::{EdgePage, EdgeQuery};
use crate::models::records::{CollectionAccess, RecordPage};
use crate::services::async_state::AsyncState;
use crate::services::backend::Backend;
use crate::services::error::StudioError;
use crate::state::explorer::{CollectionTab, GraphSub, RecordView, RecordsEpoch};
use crate::state::preferences::Preferences;
use crate::views::explorer::details::{RecordDetails, details_title};
use crate::views::explorer::graph::{EDGE_PAGE, EdgeDetails, GraphBody};
use crate::views::explorer::page::{DETAILS_SIZE, SURFACE};
use crate::views::explorer::schema_tabs::{FullTextTab, IndexesTab, SchemaTab};
use crate::views::explorer::table::RecordTable;
use crate::views::explorer::toolbar::RecordsToolbar;

/// The badges after a collection's name: its profile, its capabilities,
/// its size, and Read-only when the session cannot write.
#[component]
pub fn CollectionBadges(collection: Collection, read_only: bool) -> Element {
    let count = collection.rows.map(crate::models::format::thousands);
    rsx! {
        for b in collection.profile.header_badges() {
            span { key: "{b}", class: "badge {tone(b, collection.profile)}", "{b}" }
        }
        for a in collection.attached.iter() {
            span { key: "{a.badge()}", class: "badge {a.tone()}", "{a.badge()}" }
        }
        if let Some(n) = count {
            span { class: "badge outline", "{n} {collection.profile.short_noun()}" }
        }
        if read_only {
            span { class: "badge gray", "Read-only" }
        }
    }
}

/// The badge tone for one header badge: the family badge stays blue or
/// amber, the refining profile takes the profile's own tone.
fn tone(badge: &str, profile: Profile) -> &'static str {
    if badge == profile.badge() {
        profile.tone()
    } else {
        match badge {
            "Document" => "blue",
            _ => "gray",
        }
    }
}

/// Whether the session can read and nothing else, so the header says so.
fn read_only(access: &Option<CollectionAccess>, profile: Profile) -> bool {
    !profile.append_only()
        && access.as_ref().is_some_and(|a| {
            a.read.allowed() && !a.create.allowed() && !a.update.allowed() && !a.delete.allowed()
        })
}

#[component]
pub fn RecordWorkspace(collection: Collection, catalog: Vec<String>) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let epoch = use_context::<RecordsEpoch>();
    let mut view = use_context::<Signal<RecordView>>();
    let prefs = use_context::<Signal<Preferences>>();
    let filter_open = use_signal(|| false);
    let name = collection.name.clone();

    let schema = {
        let (backend, name) = (backend.clone(), name.clone());
        use_resource(move || {
            let (backend, name) = (backend.clone(), name.clone());
            let _conn = conn();
            async move { backend.collection_schema(&name).await }
        })
    };
    let access = {
        let (backend, name) = (backend.clone(), name.clone());
        use_resource(move || {
            let (backend, name) = (backend.clone(), name.clone());
            let _conn = conn();
            async move { backend.collection_access(&name).await }
        })
    };
    let schema_state = resource_state(&schema);
    let access_state = resource_state(&access);
    let schema_value = schema_state.loaded().cloned();
    let access_value: Option<CollectionAccess> = access_state.loaded().cloned();

    // The query this view asks. A memo, so a change that does not alter the
    // query (picking a record, switching a details tab) reads nothing.
    let query = use_memo(move || {
        let s = schema
            .read()
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .cloned()?;
        Some(view.read().query(&s))
    });
    let mut page = {
        let (backend, name) = (backend.clone(), name.clone());
        use_resource(move || {
            let (backend, name) = (backend.clone(), name.clone());
            let q = query();
            let _epoch = epoch.0.read();
            let _conn = conn();
            async move {
                match q {
                    Some(q) => backend.records(&name, &q).await,
                    // The schema has not loaded: nothing to ask yet.
                    None => std::future::pending::<Result<RecordPage, StudioError>>().await,
                }
            }
        })
    };
    let page_state = resource_state(&page);

    let selected_id = use_memo(move || {
        let chosen = view.read().record.clone();
        chosen.or_else(|| {
            page.read()
                .as_ref()
                .and_then(|r| r.as_ref().ok())
                .and_then(|p| p.rows.first().map(|r| r.id.clone()))
        })
    });
    let mut record = {
        let (backend, name) = (backend.clone(), name.clone());
        use_resource(move || {
            let (backend, name) = (backend.clone(), name.clone());
            let id = selected_id();
            let _epoch = epoch.0.read();
            let _conn = conn();
            async move {
                match id {
                    Some(id) => backend.record(&name, &id).await,
                    None => {
                        std::future::pending::<Result<crate::models::records::Record, StudioError>>(
                        )
                        .await
                    }
                }
            }
        })
    };
    let record_state = resource_state(&record);

    // Edges are read only while the Graph tab shows them.
    let edge_query = use_memo(move || {
        let v = view.read();
        (v.tab == CollectionTab::Graph).then(|| EdgeQuery {
            relation: v.relation.clone(),
            text: v.edge_text.clone(),
            page: v.edge_page,
            page_size: EDGE_PAGE,
        })
    });
    let binding = schema_value.as_ref().and_then(|s| s.graph_binding.clone());
    let mut edges = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let q = edge_query();
            let graph = schema
                .read()
                .as_ref()
                .and_then(|r| r.as_ref().ok())
                .and_then(|s| s.graph_binding.clone());
            let _epoch = epoch.0.read();
            let _conn = conn();
            async move {
                match (q, graph) {
                    (Some(q), Some(g)) => backend.edges(&g, &q).await,
                    _ => Ok(EdgePage {
                        edges: Vec::new(),
                        total: 0,
                        relations: Vec::new(),
                    }),
                }
            }
        })
    };
    let edges_state = resource_state(&edges);

    let v = view.read().clone();
    let tabs = CollectionTab::for_collection(&collection);
    let tab = if tabs.contains(&v.tab) {
        v.tab
    } else {
        CollectionTab::Data
    };
    let tab_index = tabs.iter().position(|t| *t == tab).unwrap_or(0);
    let labels: Vec<&'static str> = tabs.iter().map(|t| t.label()).collect();
    let offset = prefs.read().utc_offset_minutes;
    let observed = page_state.loaded().map(|p| {
        format!(
            "Observed {} {}",
            clock(p.observed_at, offset),
            offset_label(offset)
        )
    });
    let selected_record = record_state.loaded().cloned();
    let (prev, next) = page_state.loaded().map_or((None, None), |p| {
        let at = p
            .rows
            .iter()
            .position(|r| Some(&r.id) == selected_id.read().as_ref());
        let neighbor = |i: Option<usize>| i.and_then(|i| p.rows.get(i)).map(|r| r.id.clone());
        (
            neighbor(at.and_then(|i| i.checked_sub(1))),
            neighbor(at.map(|i| i + 1)),
        )
    });
    let can_insert = access_value.as_ref().is_some_and(|a| a.create.allowed());
    let on_graph_edges = tab == CollectionTab::Graph && v.graph_sub == GraphSub::Edges;
    let selected_edge_id = v.edge.clone().or_else(|| {
        edges_state
            .loaded()
            .and_then(|p| p.edges.first().map(|e| e.id.clone()))
    });
    let edge = edges_state.loaded().and_then(|p| {
        p.edges
            .iter()
            .find(|e| Some(&e.id) == selected_edge_id.as_ref())
            .cloned()
    });

    // The Data body: toolbar, then the table or why there is none.
    let data = rsx! {
        if let Some(schema) = schema_value.clone() {
            RecordsToolbar {
                collection: collection.clone(),
                schema: schema.clone(),
                access: access_value.clone(),
                selected: selected_record.clone(),
                filter_open,
            }
            AsyncView {
                loading: page_state.is_loading(),
                empty: false,
                error: page_state.error_message(),
                retriable: page_state.is_retriable(),
                on_retry: move |_| page.restart(),
            }
            if let Some(p) = page_state.loaded().cloned() {
                RecordTable {
                    collection: collection.clone(),
                    schema,
                    page: p,
                    selected: selected_id.read().clone(),
                    filter_open,
                    can_insert,
                }
            }
        } else {
            AsyncView {
                loading: schema_state.is_loading(),
                empty: false,
                error: schema_state.error_message(),
                retriable: schema_state.is_retriable(),
            }
        }
    };

    rsx! {
        Panel {
            surface: SURFACE,
            id: "records",
            title: collection.name.clone(),
            class: "fill",
            title_extra: rsx! {
                CollectionBadges {
                    collection: collection.clone(),
                    read_only: read_only(&access_value, collection.profile),
                }
            },
            div { class: "tabs-row",
                TabBar {
                    items: labels,
                    active: tab_index,
                    class: "collection-tabs",
                    on_select: move |i: usize| {
                        if let Some(t) = tabs.get(i) {
                            view.write().tab = *t;
                        }
                    },
                }
                if let Some(text) = observed {
                    button {
                        class: "btn ghost sm observed",
                        title: "Read the records again",
                        onclick: move |_| {
                            page.restart();
                            record.restart();
                            edges.restart();
                        },
                        Ico { icon: Icon::RefreshCw, size: 14 }
                        "{text}"
                    }
                }
            }
            match (tab, schema_value.clone()) {
                (CollectionTab::Data | CollectionTab::Dimensions | CollectionTab::Attributes | CollectionTab::Metadata, _) => data,
                (CollectionTab::Schema, Some(schema)) => rsx! { SchemaTab { collection: collection.clone(), schema } },
                (CollectionTab::Indexes, Some(schema)) => rsx! { IndexesTab { collection: collection.clone(), schema } },
                (CollectionTab::FullText, Some(schema)) => rsx! { FullTextTab { collection: collection.clone(), schema } },
                (CollectionTab::Graph, Some(schema)) => rsx! {
                    GraphBody {
                        collection: collection.clone(),
                        schema,
                        binding: binding.clone().unwrap_or_default(),
                        edges: edges_state.clone(),
                        nodes: data,
                        selected_edge: selected_edge_id.clone(),
                        access: access_value.clone(),
                        on_retry: move |_| edges.restart(),
                    }
                },
                (CollectionTab::Schema | CollectionTab::Indexes | CollectionTab::FullText | CollectionTab::Graph, None) => rsx! {
                    AsyncView {
                        loading: schema_state.is_loading(),
                        empty: false,
                        error: schema_state.error_message(),
                        retriable: schema_state.is_retriable(),
                    }
                },
            }
        }
        ResizeHandle { surface: SURFACE, target: "details", size: DETAILS_SIZE, side: Side::After }
        Panel {
            surface: SURFACE,
            id: "details",
            title: if on_graph_edges { "Edge details" } else { details_title(&collection) },
            size: DETAILS_SIZE,
            if on_graph_edges {
                if let Some(e) = edge {
                    EdgeDetails { edge: e, binding: binding.clone().unwrap_or_default(), access: access_value.clone() }
                } else {
                    div { class: "async-empty", "Select an edge to inspect it." }
                }
            } else if selected_id.read().is_none() {
                div { class: "async-empty",
                    if page_state.loaded().is_some() { "No record to inspect." } else { "Select a record to inspect it." }
                }
            } else {
                RecordDetailsBody {
                    collection: collection.clone(),
                    schema: schema_value.clone(),
                    state: record_state.clone(),
                    access: access_value.clone(),
                    prev,
                    next,
                    catalog,
                    on_retry: move |_| record.restart(),
                }
            }
        }
    }
}

#[component]
fn RecordDetailsBody(
    collection: Collection,
    schema: Option<crate::models::schema::CollectionSchema>,
    state: AsyncState<crate::models::records::Record>,
    access: Option<CollectionAccess>,
    prev: Option<String>,
    next: Option<String>,
    catalog: Vec<String>,
    on_retry: EventHandler<()>,
) -> Element {
    rsx! {
        AsyncView {
            loading: state.is_loading(),
            empty: false,
            error: state.error_message(),
            retriable: state.is_retriable(),
            on_retry: move |_| on_retry.call(()),
        }
        if let (Some(record), Some(schema)) = (state.loaded().cloned(), schema) {
            RecordDetails { collection, schema, record, access, prev, next, catalog }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::records::{Denial, Grant};

    fn access(create: bool, update: bool) -> Option<CollectionAccess> {
        let g = |ok: bool| {
            if ok {
                Grant::Allowed
            } else {
                Grant::Denied(Denial::Privilege)
            }
        };
        Some(CollectionAccess {
            role: "analyst".into(),
            create: g(create),
            read: Grant::Allowed,
            update: g(update),
            delete: g(false),
        })
    }

    #[test]
    fn a_session_that_only_reads_is_read_only() {
        assert!(read_only(&access(false, false), Profile::DocumentStrict));
        assert!(!read_only(&access(true, false), Profile::DocumentStrict));
        assert!(!read_only(&None, Profile::DocumentStrict));
    }

    #[test]
    fn an_append_only_collection_is_not_called_read_only() {
        assert!(!read_only(&access(false, false), Profile::Timeseries));
    }

    #[test]
    fn the_refining_badge_takes_the_profile_tone() {
        assert_eq!(tone("Timeseries", Profile::Timeseries), "green");
        assert_eq!(tone("Columnar", Profile::Timeseries), "gray");
        assert_eq!(tone("Document", Profile::DocumentStrict), "blue");
        assert_eq!(tone("Strict", Profile::DocumentStrict), "gray");
    }
}
