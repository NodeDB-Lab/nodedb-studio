//! The Projection tab: a seeded sample of the index, reduced to two
//! dimensions and drawn. Generate runs it. Editing a control never redraws
//! the map: the old projection stays, labeled stale, until a run succeeds.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::panel::{Panel, PanelSize, ResizeHandle, Side};
use crate::models::vector::VectorTarget;
use crate::models::vector_projection::ProjectionRequest;
use crate::routes::tabs::VectorTab;
use crate::services::backend::Backend;
use crate::state::vector::{MapView, VectorUi};
use crate::state::workspace::Surface;
use crate::views::vector::proj_map::ProjMap;
use crate::views::vector::proj_toolbar::ProjToolbar;
use crate::views::vector::record::{RecordMode, RecordPanel};

const SURFACE: Surface = Surface::Vector(VectorTab::Projection);
const RECORD_SIZE: PanelSize = PanelSize::width(340.0, 280.0, 520.0);

#[component]
pub fn ProjectionTab(target: VectorTarget) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let ui = use_context::<VectorUi>();
    let mut drafts = ui.drafts;
    let key = target.label();

    let pending = use_memo(use_reactive!(|key| drafts.read().projection(&key).pending));
    let run = {
        let backend = backend.clone();
        use_resource(move || {
            let backend = backend.clone();
            let _conn = conn();
            let pending = pending();
            async move {
                match pending {
                    Some((seq, req)) => Some((seq, backend.vector_project(&req).await)),
                    None => None,
                }
            }
        })
    };

    // A finished projection lands in the draft. A late answer to a run that
    // was canceled or replaced no longer matches `pending` and is dropped.
    use_effect(use_reactive!(|key| {
        let Some(Some((seq, outcome))) = run.read().clone() else {
            return;
        };
        let mut d = drafts.write();
        let st = d.projection_mut(&key);
        if st.pending.as_ref().map(|p| p.0) != Some(seq) {
            return;
        }
        st.pending = None;
        match outcome {
            Ok(p) => {
                st.shown = Some(Rc::new(p));
                st.error = None;
                st.view = MapView::default();
                st.focus = None;
                st.goto_note = None;
            }
            // The last projection stays, labeled as the last one.
            Err(e) => st.error = Some(e.to_string()),
        }
    }));

    let s = drafts.read().projection(&key);
    let checked = s.form.check(&target, &s.include);
    let running = pending.read().is_some();
    let stale = match (&s.shown, &checked) {
        (Some(shown), Ok(req)) => {
            // Included records are a one-run request, so they do not make a
            // projection stale.
            let mut now = req.clone();
            now.include.clone_from(&shown.request.include);
            shown.request != now
        }
        (Some(_), Err(_)) => true,
        (None, _) => false,
    };

    let generate = {
        let key = key.clone();
        move |req: ProjectionRequest| {
            let mut d = drafts.write();
            let st = d.projection_mut(&key);
            st.seq += 1;
            st.pending = Some((st.seq, req));
            st.include.clear();
            st.error = None;
        }
    };
    let cancel = {
        let key = key.clone();
        move |_| drafts.write().projection_mut(&key).pending = None
    };

    rsx! {
        div { class: "workbench vec-projection",
            Panel {
                surface: SURFACE,
                id: "projection",
                title: "Projection",
                class: "fill",
                ProjToolbar {
                    target: target.clone(),
                    running,
                    on_generate: generate.clone(),
                    on_cancel: cancel,
                }
                ProjMap {
                    target: target.clone(),
                    stale,
                    running,
                    on_generate: generate,
                }
            }
            ResizeHandle { surface: SURFACE, target: "record", size: RECORD_SIZE, side: Side::After }
            RecordPanel { surface: SURFACE, target: target.clone(), mode: RecordMode::Projection, size: RECORD_SIZE }
        }
    }
}
