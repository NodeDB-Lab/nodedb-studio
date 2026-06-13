//! Explorer: the engine-aware collection browser.
//!
//! A single NodeDB connection exposes eight storage modes. The sidebar lists
//! collections grouped by mode; selecting one swaps the main pane to that
//! mode's purpose-built viewer. The selected collection is shared between the
//! sidebar and the viewer pane via a signal owned here.

use dioxus::prelude::*;

use crate::models::collection::StorageMode;
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

#[component]
pub fn Explorer() -> Element {
    let selected = use_signal(|| Selected {
        name: "events".to_string(),
        mode: StorageMode::Document,
    });
    let sel = selected.read().clone();

    rsx! {
        div { class: "view active",
            div { class: "explorer",
                ExplorerSidebar { selected }
                div { class: "explorer-main",
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
                }
            }
        }
    }
}
