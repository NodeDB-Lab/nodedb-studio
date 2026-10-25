//! Properties for a selected collection or logical reference.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::designer::DesignSchema;
use crate::models::designer_change::{ChangeKind, PendingChange};
use crate::models::designer_ddl;
use crate::models::format::thousands;
use crate::routes::Route;
use crate::services::backend::Backend;
use crate::state::designer::{DesignerState, InspectorTab, Target};
use crate::state::explorer::{RecordView, Selected, select_collection};
use crate::state::workspace::Workspace;
use crate::views::designer::card::{index_icon, letter_tone};
use crate::views::designer::ddl_view::Ddl;
use crate::views::designer::properties::{InspectorHead, InspectorTabs};
use crate::views::designer::tasks;
use crate::views::designer::toolbar::{open_index_form, open_reference_form};

#[component]
pub fn CollectionInspector(name: String, schema: DesignSchema) -> Element {
    let nav = use_navigator();
    let mut state = use_context::<Signal<DesignerState>>();
    let mut layout = use_context::<Signal<Workspace>>();
    let mut explorer = use_context::<Signal<Option<Selected>>>();
    let mut view = use_context::<Signal<RecordView>>();
    let Some(c) = schema.collection(&name).cloned() else {
        return rsx! { div { class: "async-empty", "`{name}` is no longer in the schema." } };
    };
    let tab = state.read().tab;
    let profile = c.profile();
    let rows = c
        .collection
        .rows
        .map_or("Not reported".to_string(), thousands);
    let lines = designer_ddl::describe(&c);
    let (s1, s2, owner, owner2) = (schema.clone(), schema.clone(), name.clone(), name.clone());
    rsx! {
        div { class: "inspector",
            InspectorHead {
                icon: profile.icon(),
                kind: format!("{} collection", profile.storage_label()),
                title: c.name().to_string(),
                state: c.state,
            }
            InspectorTabs {}
            match tab {
                InspectorTab::Definition => rsx! {
                    div { class: "kv insp-kv",
                        div { class: "kv-row",
                            span { "Storage" }
                            span { class: "badge letter {letter_tone(profile)}", "{profile.letter()}" }
                        }
                        div { class: "kv-row", span { "Type" } span { "{profile.storage_label()}" } }
                        div { class: "kv-row", span { "Schema" } span { "{profile.schema_label()}" } }
                        div { class: "kv-row", span { "Records" } span { "{rows}" } }
                        if let Some(settings) = c.settings.as_ref() {
                            div { class: "kv-row", span { "Settings" } span { "{settings}" } }
                        }
                        if !c.collection.attached.is_empty() {
                            div { class: "kv-row",
                                span { "Attached" }
                                span { class: "insp-badges",
                                    for a in c.collection.attached.iter() {
                                        span { key: "{a.badge()}", class: "badge {a.tone()}", "{a.badge()}" }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "insp-label",
                        if let Some(h) = c.heading { "{h.label()}" } else { "Columns" }
                    }
                    div { class: "insp-fields",
                        for f in c.fields.iter() {
                            div { key: "{f.name()}", class: "insp-field",
                                span { class: "mono", "{f.name()}" }
                                span { class: "insp-ftype", "{f.ty().keyword()}" }
                                for flag in f.flags(profile) {
                                    span { key: "{flag.label()}", class: "badge", "{flag.label()}" }
                                }
                            }
                        }
                    }
                    if !c.indexes.is_empty() {
                        div { class: "insp-label", "Indexes" }
                        div { class: "list",
                            for i in c.indexes.iter() {
                                {
                                    let target = Target::Index(i.name.clone());
                                    rsx! {
                                        button {
                                            key: "{i.name}",
                                            class: "list-item insp-index",
                                            onclick: move |_| state.write().select(target.clone()),
                                            Ico { icon: index_icon(i), size: 16 }
                                            span { class: "list-item-main mono", "{i.name}" }
                                            span { class: "list-item-meta", "{i.summary()}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "inspector-actions",
                        button {
                            class: "btn",
                            onclick: move |_| open_index_form(&mut state, &mut layout, &s1, Some(&owner)),
                            Ico { icon: Icon::Plus, size: 16 }
                            "Add index"
                        }
                        button {
                            class: "btn",
                            onclick: move |_| open_reference_form(&mut state, &mut layout, &s2, Some(&owner2)),
                            Ico { icon: Icon::Link, size: 16 }
                            "Add reference"
                        }
                    }
                    div { class: "insp-links",
                        button {
                            class: "link-btn",
                            onclick: {
                                let name = name.clone();
                                move |_| {
                                    select_collection(&mut explorer, &mut view, &name);
                                    nav.push(Route::Explorer {});
                                }
                            },
                            Ico { icon: Icon::Table, size: 16 }
                            "Browse records in Explorer"
                        }
                    }
                },
                InspectorTab::Ddl => rsx! {
                    div { class: "insp-form",
                        Ddl { lines }
                        p { class: "insp-sub",
                            "Generated from the catalog. The server does not store the statement."
                        }
                    }
                },
            }
        }
    }
}

#[component]
pub fn ReferenceInspector(
    id: String,
    schema: DesignSchema,
    pending: Vec<PendingChange>,
) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let state = use_context::<Signal<DesignerState>>();
    let Some(r) = schema.reference(&id).cloned() else {
        return rsx! { div { class: "async-empty", "This reference is no longer in the schema." } };
    };
    let staged = pending
        .iter()
        .any(|p| p.id == id && matches!(p.kind, ChangeKind::AddReference(_)));
    let tab = state.read().tab;
    let discard = {
        let (backend, id) = (backend.clone(), id.clone());
        move |_| {
            let (backend, id) = (backend.clone(), id.clone());
            spawn_forever(async move { tasks::discard(state, backend, id).await });
        }
    };
    rsx! {
        div { class: "inspector",
            InspectorHead {
                icon: Icon::Link,
                kind: "Logical reference".to_string(),
                title: r.label(),
                state: r.state,
            }
            InspectorTabs {}
            match tab {
                InspectorTab::Definition => rsx! {
                    div { class: "kv insp-kv",
                        div { class: "kv-row", span { "Holder" } span { class: "mono", "{r.from_collection}.{r.from_field}" } }
                        div { class: "kv-row", span { "References" } span { class: "mono", "{r.to_collection}.{r.to_field}" } }
                        div { class: "kv-row", span { "Enforcement" } span { "Not enforced" } }
                        div { class: "kv-row", span { "Stored as" } span { "Workspace metadata" } }
                    }
                    p { class: "insp-sub",
                        "NodeDB does not check this reference on write. It documents the design and generates no foreign key."
                    }
                },
                InspectorTab::Ddl => rsx! {
                    div { class: "insp-form",
                        p { class: "insp-sub", "No statement. A logical reference changes no database object." }
                    }
                },
            }
            div { class: "inspector-note insp-note",
                if staged {
                    span { class: "insp-status",
                        Ico { icon: Icon::Info, size: 16 }
                        "Not applied to workspace"
                    }
                    button { class: "link-btn", onclick: discard, "Discard draft" }
                } else {
                    span { class: "insp-status",
                        Ico { icon: Icon::CircleCheck, size: 16 }
                        "Saved in the workspace"
                    }
                    button {
                        class: "link-btn",
                        disabled: true,
                        title: "Removing a reference has no designed flow yet",
                        "Remove"
                    }
                }
            }
        }
    }
}
