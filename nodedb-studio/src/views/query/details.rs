//! Query details: the selected saved query's name, owner, scope, tags,
//! version history, and what it stores about credentials.
//!
//! When a tab edits the query, the name and tags are the tab's draft and
//! an unsaved dot shows. Otherwise the facts are read-only.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::icon::{Ico, Icon};
use crate::models::format::{date_minute, date_second, offset_label};
use crate::models::saved_query::{QueryLibrary, QueryScope, SavedQuery};
use crate::services::async_state::AsyncState;
use crate::services::backend::Backend;
use crate::state::preferences::Preferences;
use crate::state::query::{QueryWorkspace, TxEpoch};
use crate::views::query::tasks::{self, Env};

#[component]
pub fn DetailsBody(library: AsyncState<QueryLibrary>) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let ws = use_context::<Signal<QueryWorkspace>>();
    let target = use_memo(move || {
        let w = ws.read();
        w.library_selected
            .clone()
            .or_else(|| w.active().and_then(|t| t.saved.clone()))
    });
    let epoch = use_memo(move || ws.read().library_epoch);
    let versions = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            let (id, _epoch, _conn) = (target(), epoch(), conn());
            async move {
                match id {
                    Some(id) => backend.saved_query_versions(&id).await,
                    None => Ok(Vec::new()),
                }
            }
        }
    });
    let versions = resource_state(&versions);

    let Some(lib) = library.loaded() else {
        return rsx! {
            AsyncView {
                loading: library.is_loading(),
                empty: library.is_empty(),
                error: library.error_message(),
                empty_message: "No saved queries yet.",
            }
        };
    };
    let Some(query) = target().and_then(|id| lib.get(&id).cloned()) else {
        return rsx! { div { class: "async-empty", "Select a saved query to see its details." } };
    };
    rsx! { QueryFacts { query, versions } }
}

#[component]
fn QueryFacts(
    query: SavedQuery,
    versions: AsyncState<Vec<crate::models::saved_query::SavedQueryVersion>>,
) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let mut ws = use_context::<Signal<QueryWorkspace>>();
    let tx = use_context::<TxEpoch>();
    let prefs = use_context::<Signal<Preferences>>();
    let env = Env::new(ws, tx);
    let offset = prefs.read().utc_offset_minutes;
    let mut adding = use_signal(|| None::<String>);
    let mut confirming = use_signal(|| false);

    // The tab that edits this query holds the draft name and tags.
    let draft = ws
        .read()
        .tab_for(&query.id)
        .map(|t| (t.id, t.title.clone(), t.tags.clone(), t.dirty()));
    let editing = draft.is_some();
    let (name, tags, unsaved) = match &draft {
        Some((_, title, tags, dirty)) => (title.clone(), tags.clone(), *dirty),
        None => (query.name.clone(), query.tags.clone(), false),
    };
    let mine = query.scope == QueryScope::MyWorkspace;
    let tab_id = draft.as_ref().map(|d| d.0);
    let version_count = versions.loaded().map_or(query.version as usize, Vec::len);

    let add_tag = move || {
        let Some(tag) = adding.peek().clone() else {
            return;
        };
        let tag = tag.trim().to_string();
        adding.set(None);
        if tag.is_empty() {
            return;
        }
        if let Some(t) = ws.write().active_mut().filter(|t| Some(t.id) == tab_id)
            && !t.tags.contains(&tag)
        {
            t.tags.push(tag);
        }
    };

    rsx! {
        div { class: "inspector query-details",
            div { class: "detail-row",
                label { r#for: "query-name", "Name" }
                input {
                    id: "query-name",
                    class: "input",
                    disabled: !editing,
                    title: if editing { "" } else { "Open the query to rename it" },
                    value: "{name}",
                    oninput: move |e| {
                        if let Some(t) = ws.write().tab_mut(tab_id.unwrap_or_default()) { t.title = e.value(); }
                    },
                }
                if unsaved {
                    span { class: "unsaved-dot blue", title: "Unsaved changes" }
                    span { class: "sr-only", " (unsaved)" }
                }
            }
            div { class: "detail-row", span { class: "detail-label", "Owner" } span { "{query.owner}" } }
            div { class: "detail-row", span { class: "detail-label", "Scope" } span { "{query.scope.label()}" } }
            div { class: "detail-row", span { class: "detail-label", "Database" } span { "{query.database}" } }
            div { class: "detail-row",
                span { class: "detail-label", "Updated" }
                span { "{date_second(query.updated_at, offset)} {offset_label(offset)}" }
            }
            div { class: "detail-row top",
                span { class: "detail-label", "Tags" }
                div { class: "tag-list",
                    for t in tags.iter().cloned() {
                        span { key: "{t}", class: "badge blue tag-chip",
                            "{t}"
                            if editing {
                                button {
                                    class: "chip-remove",
                                    "aria-label": "Remove tag {t}",
                                    onclick: {
                                        let t = t.clone();
                                        move |_| {
                                            if let Some(tab) = ws.write().tab_mut(tab_id.unwrap_or_default()) {
                                                tab.tags.retain(|x| *x != t);
                                            }
                                        }
                                    },
                                    Ico { icon: Icon::X, size: 12 }
                                }
                            }
                        }
                    }
                    if editing {
                        if let Some(text) = adding() {
                            input {
                                class: "input tag-input",
                                "aria-label": "New tag",
                                placeholder: "Tag",
                                value: "{text}",
                                oninput: move |e| adding.set(Some(e.value())),
                                onkeydown: {
                                    let mut add_tag = add_tag;
                                    move |e| match e.key() {
                                        Key::Enter => add_tag(),
                                        Key::Escape => adding.set(None),
                                        _ => {}
                                    }
                                },
                            }
                        } else {
                            button {
                                class: "icon-btn bordered sm",
                                title: "Add a tag",
                                "aria-label": "Add a tag",
                                onclick: move |_| adding.set(Some(String::new())),
                                Ico { icon: Icon::Plus, size: 14 }
                            }
                        }
                    }
                }
            }

            h4 { class: "inspector-section with-icon",
                Ico { icon: Icon::History, size: 18 }
                "Version history"
            }
            AsyncView {
                loading: versions.is_loading(),
                empty: versions.is_empty(),
                error: versions.error_message(),
                empty_message: "No stored versions.",
            }
            if let Some(list) = versions.loaded() {
                ul { class: "versions", "aria-label": "Version history",
                    for v in list.iter() {
                        li {
                            key: "{v.version}",
                            class: if v.version == query.version { "version current" } else { "version" },
                            "aria-current": if v.version == query.version { "true" } else { "false" },
                            span { class: "version-n", "v{v.version}" }
                            span { class: "version-at num", "{date_minute(v.saved_at, offset)}" }
                            span { class: "version-by", "{v.author}" }
                        }
                    }
                }
            }

            h4 { class: "inspector-section with-icon",
                Ico { icon: Icon::Lock, size: 18 }
                "Credentials"
            }
            p { strong { "Not stored" } }
            p { class: "muted", "This query does not store credentials." }

            if mine {
                div { class: "inspector-actions",
                    if confirming() {
                        div { class: "callout err confirm-delete", role: "alertdialog",
                            span {
                                "Delete " strong { "{query.name}" } " and its {version_count} versions? "
                                "Tabs that edit it keep their text as unsaved drafts."
                            }
                        }
                        button { class: "btn", onclick: move |_| confirming.set(false), "Keep" }
                        button {
                            class: "btn danger",
                            onclick: {
                                let backend = backend.clone();
                                let id = query.id.clone();
                                move |_| {
                                    confirming.set(false);
                                    let (backend, id) = (backend.clone(), id.clone());
                                    spawn_forever(async move { tasks::delete(env, backend, id).await });
                                }
                            },
                            "Delete"
                        }
                    } else {
                        button { class: "btn danger", onclick: move |_| confirming.set(true),
                            Ico { icon: Icon::Trash, size: 16 }
                            "Delete query"
                        }
                    }
                }
            }
        }
    }
}
