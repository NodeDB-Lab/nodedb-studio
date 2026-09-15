//! Explorer: the engine-aware collection browser.
//!
//! A single NodeDB connection exposes eight storage modes. The sidebar lists
//! collections grouped by mode; selecting one swaps the main pane to that
//! mode's purpose-built viewer. The selected collection is shared between the
//! sidebar and the viewer pane via a signal owned here.
//!
//! There is no fabricated default selection. Until the sidebar's seam read
//! resolves — or if it resolves to no collections at all — `selected` stays
//! `None` and the main pane says so honestly. Once `collection_groups()`
//! loads with data, `ExplorerSidebar` defaults `selected` to the first
//! collection of the first group via `default_selection` below.

use dioxus::prelude::*;

use crate::models::collection::StorageMode;
use crate::models::explorer::CollectionGroup;
use crate::views::explorer::sidebar::ExplorerSidebar;
use crate::views::explorer::viewers::document::DocumentViewer;
use crate::views::explorer::viewers::fts::FtsViewer;
use crate::views::explorer::viewers::graph::GraphViewer;
use crate::views::explorer::viewers::kv::KvViewer;
use crate::views::explorer::viewers::spatial::SpatialViewer;
use crate::views::explorer::viewers::strict::StrictViewer;
use crate::views::explorer::viewers::timeseries::TimeseriesViewer;
use crate::views::explorer::viewers::vector::VectorViewer;

/// The currently-selected collection (name + its storage mode).
#[derive(Clone, PartialEq)]
pub struct Selected {
    pub name: String,
    pub mode: StorageMode,
}

/// The Explorer's default selection: the first collection of the first
/// group, in `collection_groups()`'s own display order. `None` when there
/// are no groups (loading, empty, or errored) — there is no fallback name,
/// because any hardcoded name silently rots the moment the fixture changes
/// (see the `events` regression this replaced: the collection it named had
/// already been renamed out of the fixture).
pub fn default_selection(groups: &[CollectionGroup]) -> Option<Selected> {
    let group = groups.first()?;
    let collection = group.collections.first()?;
    Some(Selected {
        name: collection.name.clone(),
        mode: collection.mode,
    })
}

/// Whether `selected` still names a collection present in `groups`.
///
/// A reload can return a set the current pick is no longer in (renamed,
/// dropped, or a different connection). Keeping it leaves the viewer header
/// naming a collection no sidebar row matches, which is the phantom-collection
/// symptom the hardcoded default used to produce.
pub fn selection_still_present(groups: &[CollectionGroup], selected: Option<&Selected>) -> bool {
    let Some(sel) = selected else {
        return false;
    };
    groups
        .iter()
        .any(|g| g.collections.iter().any(|c| c.name == sel.name))
}

#[component]
pub fn Explorer() -> Element {
    let selected = use_signal(|| None::<Selected>);
    let sel = selected.read().clone();

    rsx! {
        div { class: "view active",
            div { class: "explorer",
                ExplorerSidebar { selected }
                div { class: "explorer-main",
                    if let Some(sel) = sel {
                        div { class: "viewer-header",
                            h2 {
                                span { "{sel.name}" }
                                span { class: "sub", "{sel.mode.key()}" }
                            }
                            div { class: "viewer-actions",
                                button { class: "btn small", "Schema" }
                                button { class: "btn small", "Indexes" }
                                button { class: "btn small", "Export" }
                                button { class: "btn small primary", "+ Insert" }
                            }
                        }
                        div { class: "viewer-body",
                            match sel.mode {
                                StorageMode::Document => rsx! { DocumentViewer {} },
                                StorageMode::Strict => rsx! { StrictViewer {} },
                                StorageMode::Vector => rsx! { VectorViewer {} },
                                StorageMode::Graph => rsx! { GraphViewer {} },
                                StorageMode::Timeseries => rsx! { TimeseriesViewer {} },
                                StorageMode::Kv => rsx! { KvViewer {} },
                                StorageMode::Spatial => rsx! { SpatialViewer {} },
                                StorageMode::Fts => rsx! { FtsViewer {} },
                            }
                        }
                    } else {
                        div { class: "async-empty", "Select a collection to view its data." }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::models::collection::Collection;
    use crate::services::connection_service::MockConnectionService;
    use crate::services::explorer_data::ExplorerData;

    fn groups_with(names: &[&str]) -> Vec<CollectionGroup> {
        vec![CollectionGroup {
            mode: StorageMode::Document,
            collections: names
                .iter()
                .map(|n| Collection {
                    name: (*n).to_string(),
                    mode: StorageMode::Document,
                    count: "1".to_string(),
                })
                .collect(),
        }]
    }

    /// A pick that survived a reload must still exist in the new set. Against
    /// an implementation that keeps any non-None selection, the second case
    /// fails and the viewer header names a collection no sidebar row matches.
    #[test]
    fn selection_is_kept_only_while_the_collection_exists() {
        let groups = groups_with(&["events", "orders"]);
        let pick = Selected {
            name: "orders".to_string(),
            mode: StorageMode::Document,
        };
        assert!(selection_still_present(&groups, Some(&pick)));

        let after_rename = groups_with(&["events", "orders_v2"]);
        assert!(
            !selection_still_present(&after_rename, Some(&pick)),
            "a reload that dropped `orders` must not keep it selected"
        );
    }

    #[test]
    fn no_selection_is_never_present() {
        assert!(!selection_still_present(&groups_with(&["events"]), None));
    }

    #[test]
    fn default_selection_is_none_for_no_groups() {
        assert!(default_selection(&[]).is_none());
    }

    #[test]
    fn default_selection_is_none_when_the_first_group_has_no_collections() {
        let groups = [CollectionGroup {
            mode: StorageMode::Document,
            collections: Vec::new(),
        }];
        assert!(default_selection(&groups).is_none());
    }

    #[test]
    fn default_selection_is_the_first_collection_of_the_first_group() {
        let groups = [
            CollectionGroup {
                mode: StorageMode::Document,
                collections: vec![
                    Collection {
                        name: "users".to_string(),
                        mode: StorageMode::Document,
                        count: "1".to_string(),
                    },
                    Collection {
                        name: "orders".to_string(),
                        mode: StorageMode::Document,
                        count: "2".to_string(),
                    },
                ],
            },
            CollectionGroup {
                mode: StorageMode::Vector,
                collections: vec![Collection {
                    name: "embeddings".to_string(),
                    mode: StorageMode::Vector,
                    count: "3".to_string(),
                }],
            },
        ];
        let selection = default_selection(&groups).expect("groups are non-empty");
        assert_eq!(selection.name, "users");
        assert_eq!(selection.mode, StorageMode::Document);
    }

    #[tokio::test]
    async fn default_selection_names_a_collection_that_actually_exists() {
        // Regression test: the Explorer used to hardcode its default
        // selection as "events" / Document, a name that does not exist in
        // `collection_groups()` — opening the Explorer showed a viewer
        // header for a collection the sidebar didn't have, with no row
        // highlighted. Reads the real fixture through the seam (not a
        // test-local stand-in like `sidebar`'s `sample_groups()`), so a
        // future fixture change can't silently reintroduce the same bug.
        let svc = MockConnectionService::ready();
        let groups = svc.collection_groups().await.expect("ready yields groups");
        let selection = default_selection(&groups).expect("fixture is non-empty");
        let exists = groups.iter().any(|g| {
            g.collections
                .iter()
                .any(|c| c.name == selection.name && c.mode == selection.mode)
        });
        assert!(
            exists,
            "default selection {:?}/{:?} must name a real collection",
            selection.name,
            selection.mode.key()
        );
    }
}
