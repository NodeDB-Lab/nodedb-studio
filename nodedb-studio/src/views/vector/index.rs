//! The Index tab: a snapshot of the vector index's counts, builds, and
//! definition. Refresh reads a new snapshot and never changes the index.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::AsyncView;
use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::models::format::{clock, offset_label};
use crate::models::vector::VectorTarget;
use crate::models::vector_index::{BuildState, Coverage, IndexStatus};
use crate::routes::Route;
use crate::routes::tabs::VectorTab;
use crate::services::async_state::AsyncState;
use crate::services::backend::Backend;
use crate::state::designer::{DesignerState, Target};
use crate::state::preferences::Preferences;
use crate::state::vector::VectorUi;
use crate::state::workspace::Surface;
use crate::views::vector::definition::DefinitionBody;
use crate::views::vector::index_status::StatusBody;

const SURFACE: Surface = Surface::Vector(VectorTab::Index);
const DEFINITION_SIZE: PanelSize = PanelSize::width(340.0, 280.0, 520.0);

#[component]
pub fn IndexTab(target: VectorTarget) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let ui = use_context::<VectorUi>();
    let prefs = use_context::<Signal<Preferences>>();
    let mut designer = use_context::<Signal<DesignerState>>();
    let nav = use_navigator();
    let mut epoch = ui.index_epoch;

    let mut read = {
        let backend = backend.clone();
        let (collection, field) = (target.collection.clone(), target.field.clone());
        use_resource(use_reactive!(|collection, field| {
            let backend = backend.clone();
            let (_conn, _epoch) = (conn(), epoch());
            async move { backend.vector_index_status(&collection, &field).await }
        }))
    };
    // While a Refresh runs, the previous snapshot stays on screen, marked.
    let refreshing = matches!(*read.state().read(), UseResourceState::Pending);
    let value = read.read().clone();
    let state = AsyncState::from_value(value.clone());
    let status: Option<IndexStatus> = state.loaded().cloned();
    let first_load = refreshing && status.is_none();
    let offset = prefs.read().utc_offset_minutes;
    let snapshot = status.as_ref().map(|s| {
        format!(
            "Snapshot {} {}",
            clock(s.snapshot_at, offset),
            offset_label(offset)
        )
    });

    rsx! {
        div { class: "workbench vec-index",
            Panel {
                surface: SURFACE,
                id: "status",
                title: "Index status",
                class: "fill",
                actions: rsx! {
                    button {
                        class: "btn",
                        disabled: refreshing,
                        title: "Read a new snapshot. The index does not change.",
                        onclick: move |_| epoch.set(epoch() + 1),
                        Ico { icon: Icon::RefreshCw, size: 16 }
                        "Refresh"
                    }
                },
                AsyncView {
                    loading: first_load,
                    empty: false,
                    error: if refreshing { None } else { state.error_message() },
                    retriable: state.is_retriable(),
                    on_retry: move |_| read.restart(),
                }
                if let Some(s) = status.clone() {
                    div { class: if refreshing { "vec-snapshot refreshing" } else { "vec-snapshot" },
                        div { class: "vec-statusline",
                            span { class: "status",
                                span { class: match s.build {
                                    BuildState::Idle => "status-dot ok",
                                    BuildState::Queued(_) | BuildState::Building => "status-dot info",
                                } }
                                "{s.build.label()}"
                            }
                            span { class: "vec-sep" }
                            span { class: "vec-snapshot-time",
                                if refreshing { "Refreshing. This is the previous snapshot." } else { "{snapshot.clone().unwrap_or_default()}" }
                            }
                        }
                        if let Coverage::Incomplete { answered, total } = s.coverage {
                            div { class: "callout warn", role: "status",
                                Ico { icon: Icon::TriangleAlert, size: 16 }
                                span { "Incomplete: {answered} of {total} shards answered. These counts are partial, not collection totals." }
                            }
                        }
                        StatusBody { status: s.clone() }
                    }
                }
            }
            ResizeHandle { surface: SURFACE, target: "definition", size: DEFINITION_SIZE, side: Side::After }
            Panel {
                surface: SURFACE,
                id: "definition",
                title: "Definition",
                size: DEFINITION_SIZE,
                if let Some(s) = status {
                    DefinitionBody {
                        definition: s.definition.clone(),
                        index: s.index.clone(),
                        on_edit: move |name: String| {
                            designer.write().select(Target::Index(name));
                            nav.push(Route::Designer {});
                        },
                    }
                } else {
                    div { class: "vec-state", "The definition shows when the index status loads." }
                }
            }
        }
    }
}
