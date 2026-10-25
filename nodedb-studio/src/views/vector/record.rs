//! The Record panel: the selected record, read-only. Content shows its
//! text fields. Vector shows its embedding.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize};
use crate::components::tabs::TabBar;
use crate::models::format::{date_second, offset_label, thousands};
use crate::models::vector::{VectorRecord, VectorTarget};
use crate::routes::Route;
use crate::routes::tabs::VectorTab;
use crate::services::backend::Backend;
use crate::state::explorer::{RecordView, Selected, select_record};
use crate::state::preferences::Preferences;
use crate::state::vector::{InspectorTab, VectorUi};
use crate::state::workspace::Surface;

/// Which tab the panel sits in, which sets its action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordMode {
    /// Use as query.
    Search,
    /// Find similar.
    Projection,
}

#[component]
pub fn RecordPanel(
    surface: Surface,
    target: VectorTarget,
    mode: RecordMode,
    size: PanelSize,
) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let ui = use_context::<VectorUi>();
    let prefs = use_context::<Signal<Preferences>>();
    let nav = use_navigator();
    let mut explorer = use_context::<Signal<Option<Selected>>>();
    let mut view = use_context::<Signal<RecordView>>();
    let mut inspector = ui.inspector;
    let mut full = ui.full_vector;
    let mut drafts = ui.drafts;

    let key = target.label();
    let id = ui.selected_id(&key);
    let record_read = {
        let backend = backend.clone();
        let (collection, field) = (target.collection.clone(), target.field.clone());
        let id = id.clone();
        use_resource(use_reactive!(|id| {
            let backend = backend.clone();
            let (collection, field) = (collection.clone(), field.clone());
            let _conn = conn();
            async move {
                match id {
                    Some(id) => backend
                        .vector_record(&collection, &field, &id)
                        .await
                        .map(Some),
                    None => Ok(None),
                }
            }
        }))
    };
    let state = resource_state(&record_read);
    let record: Option<VectorRecord> = state.loaded().cloned().flatten();
    let tab = *inspector.read();
    let offset = prefs.read().utc_offset_minutes;
    let tabs: Vec<&'static str> = InspectorTab::ALL.iter().map(|t| t.label()).collect();
    let active = InspectorTab::ALL
        .iter()
        .position(|t| *t == tab)
        .unwrap_or(0);

    rsx! {
        Panel { surface, id: "record", title: "Record", size,
            if id.is_none() {
                div { class: "vec-state",
                    match mode {
                        RecordMode::Search => "Select a neighbor to read its record.",
                        RecordMode::Projection => "Select a point on the map to read its record.",
                    }
                }
            } else {
                AsyncView {
                    loading: state.is_loading(),
                    empty: false,
                    error: state.error_message(),
                    retriable: false,
                }
                if let Some(r) = record {
                    div { class: "vec-record",
                        h3 { class: "vec-record-id mono", "{r.id}" }
                        TabBar {
                            class: "sub",
                            items: tabs,
                            active,
                            on_select: move |i: usize| {
                                if let Some(t) = InspectorTab::ALL.get(i) { inspector.set(*t); }
                            },
                        }
                        match tab {
                            InspectorTab::Content => rsx! {
                                dl { class: "vec-fields",
                                    dt { "title" }
                                    dd { "{r.title}" }
                                    dt { "language" }
                                    dd { {r.language.clone().unwrap_or_else(|| "Missing".to_string())} }
                                    dt { "body" }
                                    dd { class: "vec-body", "“{r.body}”" }
                                }
                                h4 { class: "vec-section", "Embedding" }
                                p { class: "vec-values", "{thousands(r.vector.len() as u64)} values" }
                                button {
                                    class: "vec-preview-btn mono",
                                    "aria-expanded": "{full()}",
                                    title: "Show every value on the Vector tab",
                                    onclick: move |_| inspector.set(InspectorTab::Vector),
                                    span { "{r.preview()}" }
                                    Ico { icon: Icon::ChevronRight, size: 16 }
                                }
                            },
                            InspectorTab::Vector => rsx! {
                                div { class: "kv kv-compact",
                                    div { class: "kv-row", span { "Dimensions" } span { "{r.vector.len()}" } }
                                    div { class: "kv-row", span { "Length" } span { class: "mono", "{r.norm():.4}" } }
                                    div { class: "kv-row", span { "Model" } span { {r.model.clone().unwrap_or_else(|| "Not recorded".to_string())} } }
                                    div { class: "kv-row", span { "Embedded at" }
                                        span {
                                            {r.embedded_at.map_or_else(
                                                || "Not recorded".to_string(),
                                                |t| format!("{} {}", date_second(t, offset), offset_label(offset)),
                                            )}
                                        }
                                    }
                                }
                                button {
                                    class: "btn wide vec-show-all",
                                    "aria-expanded": "{full()}",
                                    onclick: move |_| full.set(!full()),
                                    Ico { icon: Icon::List, size: 16 }
                                    if full() { "Hide all values" } else { "Show all values" }
                                }
                                if full() {
                                    pre { class: "code-block vec-all", "{r.all_values()}" }
                                }
                            },
                        }
                        div { class: "vec-actions",
                            match mode {
                                RecordMode::Search => rsx! {
                                    button {
                                        class: "btn",
                                        title: "Use this record as the query. The neighbors stay as they are until you search.",
                                        onclick: {
                                            let (key, rid) = (key.clone(), r.id.clone());
                                            move |_| {
                                                let mut d = drafts.write();
                                                let st = d.search_mut(&key);
                                                st.form.source = crate::models::vector::SourceKind::Record;
                                                st.form.record_id.clone_from(&rid);
                                            }
                                        },
                                        "Use as query"
                                    }
                                },
                                RecordMode::Projection => rsx! {
                                    button {
                                        class: "btn primary",
                                        title: "Open Search with this record as the query. Nothing runs until you press Find similar.",
                                        onclick: {
                                            let (key, rid) = (key.clone(), r.id.clone());
                                            move |_| {
                                                {
                                                    let mut d = drafts.write();
                                                    let st = d.search_mut(&key);
                                                    st.form.source = crate::models::vector::SourceKind::Record;
                                                    st.form.record_id.clone_from(&rid);
                                                }
                                                nav.push(Route::Vector { tab: VectorTab::Search });
                                            }
                                        },
                                        "Find similar"
                                    }
                                },
                            }
                            button {
                                class: "btn",
                                onclick: {
                                    let (collection, rid) = (r.collection.clone(), r.id.clone());
                                    move |_| {
                                        select_record(&mut explorer, &mut view, &collection, &rid);
                                        nav.push(Route::Explorer {});
                                    }
                                },
                                Ico { icon: Icon::FileText, size: 16 }
                                "Open in Explorer"
                            }
                        }
                    }
                }
            }
        }
    }
}
