//! A run's result on a canvas: loading, empty, and error states around the
//! graph, with the highlighted path worked out from the selection.

use dioxus::prelude::*;

use crate::components::async_view::AsyncView;
use crate::models::graph::GraphResult;
use crate::models::graph_layout::LayoutKind;
use crate::services::async_state::AsyncState;
use crate::state::graph::{CanvasState, Pick};
use crate::views::graph::canvas::GraphCanvas;

#[component]
pub fn ResultCanvas(
    state: AsyncState<GraphResult>,
    layout: LayoutKind,
    canvas: Signal<CanvasState>,
    on_retry: EventHandler<()>,
    empty_message: String,
    #[props(default = false)] counts: bool,
) -> Element {
    let c = canvas.read().clone();
    rsx! {
        div { class: "graph-region",
            AsyncView {
                loading: state.is_loading(),
                empty: state.is_empty(),
                error: state.error_message(),
                retriable: state.is_retriable(),
                on_retry: move |_| on_retry.call(()),
                empty_message,
            }
            if let Some(result) = state.loaded() {
                {
                    let visible = c.visible(result);
                    let pick = c.effective_pick(&visible);
                    let highlight = match &pick {
                        Some(Pick::Node(p)) => c.highlighted(result, p),
                        Some(Pick::Edge(_)) | None => None,
                    };
                    rsx! {
                        GraphCanvas {
                            graph: visible,
                            layout_kind: layout,
                            canvas,
                            selected: pick,
                            highlight,
                            counts,
                        }
                    }
                }
            }
        }
    }
}
