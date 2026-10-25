//! The Properties panel: the selected object's definition and DDL, or the
//! form that stages a new index or reference.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::components::panel::{Panel, PanelSize};
use crate::models::designer::{DesignSchema, ObjectState};
use crate::models::designer_change::{ChangeKind, PendingChange};
use crate::state::designer::{DesignerState, Inspector, InspectorTab, Target};
use crate::views::designer::properties_collection::{CollectionInspector, ReferenceInspector};
use crate::views::designer::properties_index::{IndexInspector, NewIndexForm, NewReferenceForm};

pub const SURFACE: crate::state::workspace::Surface = crate::state::workspace::Surface::Designer;
pub const PROPERTIES_SIZE: PanelSize = PanelSize::width(300.0, 280.0, 520.0);

/// The heading of an inspector: what kind of object, its name, its state.
#[component]
pub fn InspectorHead(icon: Icon, kind: String, title: String, state: ObjectState) -> Element {
    rsx! {
        div { class: "insp-head",
            div { class: "insp-kind",
                span { class: "insp-glyph", Ico { icon, size: 18 } }
                span { "{kind}" }
            }
            div { class: "insp-title",
                h3 { "{title}" }
                if state.is_draft() {
                    span { class: "badge outline", "Draft" }
                }
            }
        }
    }
}

/// Definition and DDL.
#[component]
pub fn InspectorTabs() -> Element {
    let mut state = use_context::<Signal<DesignerState>>();
    let active = state.read().tab;
    rsx! {
        div { class: "tab-bar sub insp-tabs", role: "tablist",
            for tab in InspectorTab::ALL {
                button {
                    key: "{tab.label()}",
                    class: if tab == active { "tab active" } else { "tab" },
                    role: "tab",
                    "aria-selected": "{tab == active}",
                    onclick: move |_| state.write().tab = tab,
                    "{tab.label()}"
                }
            }
        }
    }
}

/// The staged change that creates index `name`, if one does.
pub fn change_of_index<'a>(pending: &'a [PendingChange], name: &str) -> Option<&'a PendingChange> {
    pending
        .iter()
        .find(|p| matches!(&p.kind, ChangeKind::CreateIndex(i) if i.name == name))
}

#[component]
pub fn PropertiesPanel(schema: DesignSchema, pending: Vec<PendingChange>) -> Element {
    let state = use_context::<Signal<DesignerState>>();
    let (inspector, selected) = {
        let s = state.read();
        (s.inspector.clone(), s.selected.clone())
    };
    rsx! {
        Panel {
            surface: SURFACE,
            id: "properties",
            title: "Properties",
            size: PROPERTIES_SIZE,
            match inspector {
                Inspector::NewIndex(form) => rsx! { NewIndexForm { form, schema } },
                Inspector::NewReference(form) => rsx! { NewReferenceForm { form, schema } },
                Inspector::Selection => match selected {
                    Some(Target::Index(name)) => rsx! { IndexInspector { name, schema, pending } },
                    Some(Target::Collection(name)) => rsx! { CollectionInspector { name, schema } },
                    Some(Target::Reference(id)) => rsx! { ReferenceInspector { id, schema, pending } },
                    None => rsx! {
                        div { class: "async-empty",
                            "Select a collection, index, or reference to see its definition."
                        }
                    },
                },
            }
        }
    }
}
