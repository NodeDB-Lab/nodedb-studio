//! The Designer: a schema and index workbench. Collections, fields, indexes,
//! and logical references draw on a canvas, and Properties edits the
//! selected object.
//!
//! Every edit is staged first. The database changes only after the review
//! panel and Apply.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::async_view::{AsyncView, resource_state};
use crate::components::panel::{ResizeHandle, Side};
use crate::models::designer::DesignSchema;
use crate::services::backend::Backend;
use crate::state::designer::DesignerState;
use crate::state::workspace::Workspace;
use crate::views::designer::canvas::Canvas;
use crate::views::designer::create::CreateDrawer;
use crate::views::designer::properties::{PROPERTIES_SIZE, PropertiesPanel};
use crate::views::designer::review::ReviewPanel;
use crate::views::designer::toolbar::{SURFACE, Toolbar, open_index_form, open_reference_form};

#[component]
pub fn DesignerPage() -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut state = use_context::<Signal<DesignerState>>();
    let mut layout = use_context::<Signal<Workspace>>();
    let epoch = use_memo(move || state.read().epoch);

    let mut schema = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            let (_conn, _epoch) = (conn(), epoch());
            async move { backend.design_schema().await }
        }
    });
    let pending = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            let (_conn, _epoch) = (conn(), epoch());
            async move { backend.pending_changes().await }
        }
    });
    let mut review = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            let (_conn, _epoch) = (conn(), epoch());
            async move { backend.review_changes().await }
        }
    });

    let schema_state = resource_state(&schema);
    let review_state = resource_state(&review);
    let applied: DesignSchema = schema_state.loaded().cloned().unwrap_or_default();
    let staged = pending
        .read()
        .clone()
        .and_then(Result::ok)
        .unwrap_or_default();
    // What the canvas draws: the database, then the staged changes over it.
    let drawn = applied.with_pending(&staged);
    let (create, review_open) = {
        let s = state.read();
        (s.create.clone(), s.review.clone())
    };
    let drawer_open = review_open.is_some();
    let database = applied.database.clone();

    rsx! {
        div { class: "page-body designer-page",
            Toolbar { schema: drawn.clone(), pending: staged.len(), database }
            div { class: "workbench designer-body",
                if schema_state.loaded().is_some() {
                    Canvas {
                        schema: drawn.clone(),
                        on_add_index: {
                            let drawn = drawn.clone();
                            move |name: String| open_index_form(&mut state, &mut layout, &drawn, Some(&name))
                        },
                        on_add_reference: {
                            let drawn = drawn.clone();
                            move |name: String| open_reference_form(&mut state, &mut layout, &drawn, Some(&name))
                        },
                    }
                } else {
                    div { class: "designer-canvas",
                        AsyncView {
                            loading: schema_state.is_loading(),
                            empty: false,
                            error: schema_state.error_message(),
                            retriable: schema_state.is_retriable(),
                            on_retry: move |_| schema.restart(),
                        }
                    }
                }
                if let Some(rs) = review_open {
                    ReviewPanel {
                        pending: staged.clone(),
                        review: review_state,
                        state_of: rs,
                        on_retry: move |_| review.restart(),
                    }
                }
                if !drawer_open {
                    ResizeHandle { surface: SURFACE, target: "properties", size: PROPERTIES_SIZE, side: Side::After }
                    PropertiesPanel { schema: drawn.clone(), pending: staged.clone() }
                }
                if let Some(c) = create {
                    CreateDrawer { schema: drawn, create: c }
                }
            }
        }
    }
}
