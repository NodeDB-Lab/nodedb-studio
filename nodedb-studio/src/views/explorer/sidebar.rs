//! Explorer sidebar: collections grouped by storage mode, read through the
//! seam. Split into a fetch wrapper (`ExplorerSidebar`, owns the async read)
//! and a pure presentational component (`SidebarGroups`, takes `AsyncState`
//! as input) so the four states are render-testable without a runtime.
//!
//! Clicking a collection updates the shared selection, which swaps the
//! viewer pane. `ExplorerSidebar` also owns defaulting that selection: once
//! `collection_groups()` loads with data and nothing is selected yet, it
//! picks the first collection of the first group (`default_selection`) —
//! there is no selection at all while loading, empty, or errored.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::AsyncView;
use crate::models::explorer::CollectionGroup;
use crate::services::async_state::AsyncState;
use crate::services::backend::Backend;
use crate::views::explorer::{Selected, default_selection};

#[component]
pub fn ExplorerSidebar(selected: Signal<Option<Selected>>) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let mut groups = use_resource(move || {
        let backend = backend.clone();
        async move { backend.collection_groups().await }
    });

    // Clone the resource value out of its guard immediately — never hold a
    // read guard across an await; there is none here.
    let state = AsyncState::from_value(groups.read().clone());

    // Default the selection once real data is in, but only while nothing has
    // been picked yet — a later reload (`on_retry`) must never clobber a
    // selection the user already made. Reads `groups` (the resource itself,
    // not the derived `state` local) inside the effect so it reruns exactly
    // when the resource changes; `selected.peek()` reads without subscribing.
    use_effect(move || {
        let value = groups.read().clone();
        if selected.peek().is_some() {
            return;
        }
        if let Some(Ok(gs)) = value
            && let Some(first) = default_selection(&gs)
        {
            selected.set(Some(first));
        }
    });

    rsx! {
        SidebarGroups { state, selected, on_retry: move |_| groups.restart() }
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct SidebarGroupsProps {
    pub state: AsyncState<Vec<CollectionGroup>>,
    pub selected: Signal<Option<Selected>>,
    #[props(default)]
    pub on_retry: EventHandler<()>,
}

#[component]
pub fn SidebarGroups(props: SidebarGroupsProps) -> Element {
    let state = &props.state;
    let mut selected = props.selected;

    rsx! {
        aside { class: "explorer-sidebar",
            div { class: "explorer-toolbar",
                input { placeholder: "Filter collections…" }
                button { class: "btn small ghost", title: "New collection", "+" }
            }
            AsyncView {
                loading: state.is_loading(),
                empty: state.is_empty(),
                error: state.error_message(),
                retriable: state.is_retriable(),
                on_retry: move |_| props.on_retry.call(()),
                empty_message: "No collections.".to_string(),
            }
            if let Some(groups) = state.loaded() {
                for group in groups {
                    div { key: "{group.mode.key()}", class: "engine-group",
                        div { class: "engine-group-header",
                            span { class: "chev", "▾" }
                            " {group.mode.label().to_uppercase()}"
                        }
                        for col in &group.collections {
                            {
                                let sel = selected.read();
                                let is_active = sel
                                    .as_ref()
                                    .is_some_and(|s| s.name == col.name && s.mode == col.mode);
                                drop(sel);
                                let item_class = if is_active { "collection active" } else { "collection" };
                                let name = col.name.clone();
                                let mode = col.mode;
                                rsx! {
                                    div {
                                        key: "{col.name}",
                                        class: "{item_class}",
                                        onclick: move |_| selected.set(Some(Selected { name: name.clone(), mode })),
                                        span { class: "ico", "{col.mode.icon_letter()}" }
                                        " {col.name} "
                                        span { class: "count", "{col.count}" }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::collection::{Collection, StorageMode};
    use crate::services::error::StudioError;

    fn render(app: fn() -> Element) -> String {
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        dioxus_ssr::render(&dom)
    }

    fn sample_groups() -> Vec<CollectionGroup> {
        vec![
            CollectionGroup {
                mode: StorageMode::Document,
                collections: vec![Collection {
                    name: "users".to_string(),
                    mode: StorageMode::Document,
                    count: "12,481".to_string(),
                }],
            },
            CollectionGroup {
                mode: StorageMode::Vector,
                collections: vec![Collection {
                    name: "embeddings".to_string(),
                    mode: StorageMode::Vector,
                    count: "2.4M".to_string(),
                }],
            },
        ]
    }

    fn selected_signal() -> Signal<Option<Selected>> {
        Signal::new(Some(Selected {
            name: "users".to_string(),
            mode: StorageMode::Document,
        }))
    }

    fn no_selection_signal() -> Signal<Option<Selected>> {
        Signal::new(None)
    }

    fn app_loading() -> Element {
        rsx! {
            SidebarGroups {
                state: AsyncState::Loading,
                selected: selected_signal(),
            }
        }
    }

    fn app_empty() -> Element {
        rsx! {
            SidebarGroups {
                state: AsyncState::from_value(Some(Ok(Vec::<CollectionGroup>::new()))),
                selected: selected_signal(),
            }
        }
    }

    fn app_error() -> Element {
        rsx! {
            SidebarGroups {
                state: AsyncState::Error(StudioError::NotConnected),
                selected: selected_signal(),
            }
        }
    }

    fn app_ready() -> Element {
        rsx! {
            SidebarGroups {
                state: AsyncState::from_value(Some(Ok(sample_groups()))),
                selected: selected_signal(),
            }
        }
    }

    fn app_ready_no_selection() -> Element {
        rsx! {
            SidebarGroups {
                state: AsyncState::from_value(Some(Ok(sample_groups()))),
                selected: no_selection_signal(),
            }
        }
    }

    #[test]
    fn loading_state_renders_spinner() {
        let html = render(app_loading);
        assert!(html.contains("async-loading"));
    }

    #[test]
    fn empty_state_renders_message() {
        let html = render(app_empty);
        assert!(html.contains("async-empty"));
        assert!(html.contains("No collections"));
    }

    #[test]
    fn error_state_renders_error() {
        let html = render(app_error);
        assert!(html.contains("async-error"));
    }

    #[test]
    fn ready_state_renders_keyed_groups_and_collections() {
        let html = render(app_ready);
        assert!(html.contains("engine-group"));
        assert!(html.contains("DOCUMENT"));
        assert!(html.contains("VECTOR"));
        assert!(html.contains("users"));
        assert!(html.contains("embeddings"));
        // The selected collection ("users") renders with the active class.
        assert!(html.contains("collection active"));
    }

    #[test]
    fn no_selection_highlights_no_row() {
        // Honest "nothing picked yet" state: no row gets the active class.
        let html = render(app_ready_no_selection);
        assert!(!html.contains("collection active"));
    }
}
