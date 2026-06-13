//! Explorer sidebar: collections grouped by storage mode. Clicking a
//! collection updates the shared selection, which swaps the viewer pane.

use dioxus::prelude::*;

use crate::data::mock;
use crate::models::collection::{Collection, StorageMode};
use crate::views::explorer::Selected;

#[component]
pub fn ExplorerSidebar(selected: Signal<Selected>) -> Element {
    // Group collections by mode, preserving the mock's order.
    let collections = use_hook(mock::explorer_collections);
    let mut groups: Vec<(StorageMode, Vec<Collection>)> = Vec::new();
    for col in &collections {
        match groups.last_mut() {
            Some((mode, items)) if *mode == col.mode => items.push(col.clone()),
            _ => groups.push((col.mode, vec![col.clone()])),
        }
    }

    rsx! {
        aside { class: "explorer-sidebar",
            div { class: "explorer-toolbar",
                input { placeholder: "Filter collections…" }
                button { class: "btn small ghost", title: "New collection", "+" }
            }
            for (mode, items) in groups {
                div { class: "engine-group",
                    div { class: "engine-group-header",
                        span { class: "chev", "▾" }
                        " {mode.label().to_uppercase()}"
                    }
                    for col in items {
                        {
                            let sel = selected.read();
                            let is_active = sel.name == col.name && sel.mode == col.mode;
                            drop(sel);
                            let item_class = if is_active { "collection active" } else { "collection" };
                            let name = col.name.clone();
                            let mode = col.mode;
                            rsx! {
                                div {
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
