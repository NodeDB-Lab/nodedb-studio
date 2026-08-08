//! Explorer sidebar: collections grouped by storage mode, read through the
//! seam. Split into a fetch wrapper (`ExplorerSidebar`, owns the async read)
//! and a pure presentational component (`SidebarGroups`, takes `AsyncState`
//! as input) so the four states are render-testable without a runtime.
//!
//! Clicking a collection updates the shared selection, which swaps the
//! viewer pane.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::AsyncView;
use crate::models::explorer::CollectionGroup;
use crate::services::async_state::AsyncState;
use crate::services::backend::Backend;
use crate::views::explorer::Selected;

#[component]
pub fn ExplorerSidebar(selected: Signal<Selected>) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let mut groups = use_resource(move || {
        let backend = backend.clone();
        async move { backend.collection_groups().await }
    });

    // Clone the resource value out of its guard immediately — never hold a
    // read guard across an await; there is none here.
    let state = AsyncState::from_value(groups.read().clone());

    rsx! {
        SidebarGroups { state, selected, on_retry: move |_| groups.restart() }
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct SidebarGroupsProps {
    pub state: AsyncState<Vec<CollectionGroup>>,
    pub selected: Signal<Selected>,
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
                                let is_active = sel.name == col.name && sel.mode == col.mode;
                                drop(sel);
                                let item_class = if is_active { "collection active" } else { "collection" };
                                let name = col.name.clone();
                                let mode = col.mode;
                                rsx! {
                                    div {
                                        key: "{col.name}",
                                        class: "{item_class}",
                                        onclick: move |_| selected.set(Selected { name: name.clone(), mode }),
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

    fn selected_signal() -> Signal<Selected> {
        Signal::new(Selected {
            name: "users".to_string(),
            mode: StorageMode::Document,
        })
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
}
