//! The Designer toolbar: add an object, arrange the canvas, and review.
//! One row.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::designer::DesignSchema;
use crate::models::designer_form::{IndexForm, ReferenceForm};
use crate::services::backend::Backend;
use crate::state::designer::{CreateState, DesignerState, Inspector, ReviewState};
use crate::state::workspace::{Surface, Workspace};
use crate::views::designer::tasks;

pub const SURFACE: Surface = Surface::Designer;

/// Open the New index form in Properties, on `collection` when given.
pub fn open_index_form(
    state: &mut Signal<DesignerState>,
    layout: &mut Signal<Workspace>,
    schema: &DesignSchema,
    collection: Option<&str>,
) {
    layout.write().set_visible(SURFACE, "properties", true);
    let mut s = state.write();
    s.create = None;
    s.review = None;
    s.form_error = None;
    s.inspector = Inspector::NewIndex(IndexForm::new(schema, collection));
}

pub fn open_reference_form(
    state: &mut Signal<DesignerState>,
    layout: &mut Signal<Workspace>,
    schema: &DesignSchema,
    from: Option<&str>,
) {
    layout.write().set_visible(SURFACE, "properties", true);
    let mut s = state.write();
    s.create = None;
    s.review = None;
    s.form_error = None;
    s.inspector = Inspector::NewReference(ReferenceForm::new(schema, from));
}

#[component]
pub fn Toolbar(schema: DesignSchema, pending: usize, database: String) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let mut state = use_context::<Signal<DesignerState>>();
    let mut layout = use_context::<Signal<Workspace>>();
    let notice = state.read().notice.clone();
    let none_pending = pending == 0;
    let no_collections = schema.collections.is_empty();

    let sc = schema.clone();
    let sr = schema.clone();
    rsx! {
        div { class: "toolbar nowrap designer-toolbar",
            button {
                class: "btn",
                onclick: move |_| {
                    let mut s = state.write();
                    s.review = None;
                    s.create = Some(CreateState::new(&database));
                },
                Ico { icon: Icon::Plus, size: 16 }
                "Collection"
            }
            button {
                class: "btn",
                disabled: no_collections,
                title: if no_collections { "Create a collection first" } else { "" },
                onclick: move |_| open_index_form(&mut state, &mut layout, &sc, None),
                Ico { icon: Icon::Plus, size: 16 }
                "Index"
            }
            button {
                class: "btn",
                disabled: no_collections,
                title: if no_collections { "Create a collection first" } else { "" },
                onclick: move |_| open_reference_form(&mut state, &mut layout, &sr, None),
                Ico { icon: Icon::Link, size: 16 }
                "Add reference"
            }
            if let Some(text) = notice {
                span { class: "designer-notice", role: "status", "{text}" }
            }
            div { class: "toolbar-spacer" }
            button {
                class: "btn",
                title: "Place every card by the automatic layout",
                onclick: move |_| layout.write().clear_card_positions(),
                Ico { icon: Icon::Network, size: 16 }
                "Auto-layout"
            }
            button {
                class: "btn",
                disabled: none_pending,
                title: if none_pending { "No staged changes to export" } else { "Copy the DDL of the staged changes" },
                onclick: {
                    let backend = backend.clone();
                    move |_| {
                        let backend = backend.clone();
                        spawn_forever(async move { tasks::export_ddl(state, backend).await });
                    }
                },
                Ico { icon: Icon::Download, size: 16 }
                "Export DDL"
            }
            button {
                class: "btn primary",
                disabled: none_pending,
                title: if none_pending { "Stage a change to review it" } else { "Review the staged changes before applying them" },
                onclick: move |_| {
                    let mut s = state.write();
                    s.create = None;
                    s.review = Some(ReviewState::default());
                },
                Ico { icon: Icon::FileText, size: 16 }
                "Review changes ({pending})"
            }
        }
    }
}
