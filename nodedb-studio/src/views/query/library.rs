//! The Saved queries panel: folders, My queries, Shared, and the queries
//! whose last run failed. A row selects, a double click opens.

use std::collections::BTreeSet;
use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::async_view::AsyncView;
use crate::components::icon::{Ico, Icon};
use crate::models::saved_query::{QueryLibrary, SavedQuery};
use crate::services::async_state::AsyncState;
use crate::services::backend::Backend;
use crate::state::query::{QueryWorkspace, SidePane, TxEpoch};
use crate::views::query::tasks::{self, Env};

/// One group of the library, after the search.
struct Group<'a> {
    id: String,
    label: String,
    items: Vec<&'a SavedQuery>,
    /// A user folder shows even when empty and unfiltered.
    folder: bool,
}

fn groups<'a>(lib: &'a QueryLibrary, needle: &str) -> Vec<Group<'a>> {
    let keep = |v: Vec<&'a SavedQuery>| -> Vec<&'a SavedQuery> {
        v.into_iter()
            .filter(|q| QueryLibrary::matches(q, needle))
            .collect()
    };
    let searching = !needle.trim().is_empty();
    let mut out = vec![Group {
        id: "mine".into(),
        label: "My queries".into(),
        items: keep(lib.mine()),
        folder: false,
    }];
    for f in &lib.folders {
        out.push(Group {
            id: format!("folder:{f}"),
            label: f.clone(),
            items: keep(lib.in_folder(f)),
            folder: true,
        });
    }
    out.push(Group {
        id: "shared".into(),
        label: "Shared".into(),
        items: keep(lib.shared()),
        folder: false,
    });
    out.push(Group {
        id: "errors".into(),
        label: "Recent errors".into(),
        items: keep(lib.recent_errors()),
        folder: false,
    });
    out.retain(|g| {
        !g.items.is_empty() || (g.folder && !searching) || (g.id == "mine" && !searching)
    });
    out
}

#[component]
pub fn LibraryBody(library: AsyncState<QueryLibrary>, on_retry: EventHandler<()>) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let mut ws = use_context::<Signal<QueryWorkspace>>();
    let tx = use_context::<TxEpoch>();
    let env = Env::new(ws, tx);
    let mut filter = use_signal(String::new);
    let mut adding = use_signal(|| None::<String>);
    let mut closed = use_signal(BTreeSet::<String>::new);
    let (selected, notice_dirty): (Option<String>, BTreeSet<String>) = {
        let w = ws.read();
        (
            w.library_selected.clone(),
            w.tabs()
                .iter()
                .filter(|t| t.dirty())
                .filter_map(|t| t.saved.clone())
                .collect(),
        )
    };
    let needle = filter.read().clone();

    let create = {
        let backend = backend.clone();
        move || {
            let Some(name) = adding.peek().clone() else {
                return;
            };
            if name.trim().is_empty() {
                return;
            }
            adding.set(None);
            let backend = backend.clone();
            spawn_forever(async move { tasks::create_folder(env, backend, name).await });
        }
    };

    rsx! {
        div { class: "toolbar nowrap library-actions",
            button {
                class: "btn primary",
                onclick: move |_| adding.set(Some(String::new())),
                Ico { icon: Icon::FolderPlus, size: 16 }
                "New folder"
            }
        }
        if let Some(name) = adding() {
            div { class: "toolbar nowrap folder-row",
                input {
                    class: "input",
                    "aria-label": "Folder name",
                    placeholder: "Folder name",
                    value: "{name}",
                    oninput: move |e| adding.set(Some(e.value())),
                    onkeydown: {
                        let mut create = create.clone();
                        move |e| match e.key() {
                            Key::Enter => create(),
                            Key::Escape => adding.set(None),
                            _ => {}
                        }
                    },
                }
                button {
                    class: "btn sm primary",
                    disabled: name.trim().is_empty(),
                    onclick: { let mut create = create.clone(); move |_| create() },
                    "Create"
                }
                button { class: "btn sm", onclick: move |_| adding.set(None), "Cancel" }
            }
        }
        div { class: "toolbar nowrap library-search",
            div { class: "search-field grow",
                Ico { icon: Icon::Search, size: 16 }
                input {
                    class: "input",
                    placeholder: "Search queries\u{2026}",
                    "aria-label": "Search saved queries",
                    value: "{filter}",
                    oninput: move |e| filter.set(e.value()),
                }
            }
        }
        AsyncView {
            loading: library.is_loading(),
            empty: library.is_empty(),
            error: library.error_message(),
            retriable: library.is_retriable(),
            on_retry: move |_| on_retry.call(()),
            empty_message: "No saved queries yet. Write a statement and choose Save as.",
        }
        if let Some(lib) = library.loaded() {
            div { class: "library", role: "listbox", "aria-label": "Saved queries",
                for g in groups(lib, &needle) {
                    {
                        let gid = g.id.clone();
                        let collapsed = closed.read().contains(&g.id) && needle.trim().is_empty();
                        let is_folder = g.folder;
                        let is_errors = g.id == "errors";
                        let empty = g.items.is_empty();
                        rsx! {
                            div { key: "{g.id}", class: "library-group",
                                button {
                                    class: "library-head",
                                    "aria-expanded": "{!collapsed}",
                                    onclick: move |_| {
                                        let mut set = closed.write();
                                        if !set.remove(&gid) { set.insert(gid.clone()); }
                                    },
                                    if is_folder {
                                        Ico { icon: Icon::Folder, size: 16 }
                                    } else {
                                        Ico { icon: if collapsed { Icon::ChevronRight } else { Icon::ChevronDown }, size: 16 }
                                    }
                                    span { "{g.label}" }
                                }
                                if !collapsed {
                                    if empty && is_folder {
                                        div { class: "library-empty", "No queries in this folder." }
                                    }
                                    for q in g.items.iter().copied() {
                                        {
                                            let id = q.id.clone();
                                            let is_selected = selected.as_deref() == Some(q.id.as_str());
                                            let unsaved = notice_dirty.contains(&q.id);
                                            let open_q = q.clone();
                                            rsx! {
                                                button {
                                                    key: "{q.id}",
                                                    class: if is_selected { "list-item library-item selected" } else { "list-item library-item" },
                                                    role: "option",
                                                    "aria-selected": "{is_selected}",
                                                    title: q.last_error.clone().unwrap_or_default(),
                                                    onclick: move |_| {
                                                        let mut w = ws.write();
                                                        w.library_selected = Some(id.clone());
                                                        w.side = SidePane::Details;
                                                    },
                                                    ondoubleclick: move |_| {
                                                        ws.write().open_saved(&open_q);
                                                    },
                                                    if is_errors {
                                                        span { class: "state-icon err", Ico { icon: Icon::CircleAlert, size: 16 } }
                                                    } else {
                                                        Ico { icon: Icon::FileText, size: 16 }
                                                    }
                                                    span { class: "list-item-main", "{q.name}" }
                                                    if unsaved {
                                                        span { class: "unsaved-dot blue", title: "Unsaved changes" }
                                                        span { class: "sr-only", " (unsaved)" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if !needle.trim().is_empty() && groups(lib, &needle).iter().all(|g| g.items.is_empty()) {
                    div { class: "async-empty", "No saved query matches \u{201c}{needle}\u{201d}." }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::MockConnectionService;
    use crate::services::workbench_data::WorkbenchData;

    #[tokio::test]
    async fn groups_follow_the_design_order() {
        let lib = MockConnectionService::ready()
            .query_library()
            .await
            .expect("library");
        let ids: Vec<String> = groups(&lib, "").into_iter().map(|g| g.id).collect();
        assert_eq!(ids, ["mine", "shared", "errors"]);
    }

    #[tokio::test]
    async fn a_search_hides_groups_it_empties_and_matches_tags() {
        let lib = MockConnectionService::ready()
            .query_library()
            .await
            .expect("library");
        let ids: Vec<String> = groups(&lib, "revenue").into_iter().map(|g| g.id).collect();
        assert_eq!(ids, ["mine"], "only Customer spend carries the tag");
        assert!(groups(&lib, "zzz").is_empty());
    }

    #[tokio::test]
    async fn a_folder_shows_while_empty_but_not_during_a_search() {
        let svc = MockConnectionService::ready();
        svc.create_query_folder("Finance").await.expect("folder");
        let lib = svc.query_library().await.expect("library");
        let ids: Vec<String> = groups(&lib, "").into_iter().map(|g| g.id).collect();
        assert_eq!(ids, ["mine", "folder:Finance", "shared", "errors"]);
        let ids: Vec<String> = groups(&lib, "sales").into_iter().map(|g| g.id).collect();
        assert_eq!(ids, ["mine"]);
    }
}
