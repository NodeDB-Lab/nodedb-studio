//! Index and reference forms in Properties: a staged index edited in place,
//! and the forms that stage a new one.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::models::designer::{DesignSchema, IndexChoice, VectorMetric};
use crate::models::designer_change::PendingChange;
use crate::models::designer_ddl;
use crate::models::designer_form::{IndexForm, ReferenceForm};
use crate::services::backend::Backend;
use crate::state::designer::{DesignerState, Inspector, InspectorTab};
use crate::views::designer::card::index_icon;
use crate::views::designer::ddl_view::Ddl;
use crate::views::designer::properties::{InspectorHead, InspectorTabs, change_of_index};
use crate::views::designer::tasks;

fn metric_of(label: &str) -> VectorMetric {
    VectorMetric::ALL
        .into_iter()
        .find(|m| m.ddl() == label)
        .unwrap_or(VectorMetric::Cosine)
}

fn choice_of(label: &str) -> IndexChoice {
    IndexChoice::ALL
        .into_iter()
        .find(|c| c.label() == label)
        .unwrap_or(IndexChoice::Secondary)
}

/// Name, collection, field, and index type.
#[component]
fn IndexFields(
    form: IndexForm,
    schema: DesignSchema,
    disabled: bool,
    on_name: EventHandler<String>,
    on_change: EventHandler<IndexForm>,
) -> Element {
    let owner = schema.collection(&form.collection);
    let fields: Vec<String> = owner
        .map(|c| {
            c.fields_for(form.choice)
                .iter()
                .map(|f| f.name().to_string())
                .collect()
        })
        .unwrap_or_default();
    let edit = {
        let (form, schema) = (form.clone(), schema.clone());
        move |change: Box<dyn FnOnce(&mut IndexForm)>| {
            let mut next = form.clone();
            next.edit(&schema, change);
            on_change.call(next);
        }
    };
    let (e1, e2, e3, e4) = (edit.clone(), edit.clone(), edit.clone(), edit);
    rsx! {
        div { class: "field",
            label { r#for: "ix-name", "Name" }
            input {
                id: "ix-name",
                class: "input mono",
                value: "{form.name}",
                disabled,
                spellcheck: "false",
                onchange: move |e| on_name.call(e.value()),
            }
        }
        div { class: "field",
            label { r#for: "ix-collection", "Collection" }
            select {
                id: "ix-collection",
                class: "select native",
                disabled,
                onchange: move |e| {
                    let v = e.value();
                    e1(Box::new(move |f| f.collection = v));
                },
                for c in schema.collections.iter() {
                    option {
                        key: "{c.name()}",
                        value: "{c.name()}",
                        selected: c.name() == form.collection,
                        "{c.name()}"
                    }
                }
            }
        }
        div { class: "field",
            label { r#for: "ix-field", "Field" }
            select {
                id: "ix-field",
                class: "select native",
                disabled: disabled || fields.is_empty(),
                onchange: move |e| {
                    let v = e.value();
                    e2(Box::new(move |f| f.field = v));
                },
                if fields.is_empty() {
                    option { value: "", selected: true, "No field fits" }
                }
                for f in fields.iter() {
                    option { key: "{f}", value: "{f}", selected: *f == form.field, "{f}" }
                }
            }
        }
        div { class: "field",
            label { r#for: "ix-type", "Index type" }
            select {
                id: "ix-type",
                class: "select native",
                disabled,
                onchange: move |e| {
                    let v = choice_of(&e.value());
                    e3(Box::new(move |f| f.choice = v));
                },
                for c in IndexChoice::ALL {
                    option {
                        key: "{c.label()}",
                        value: "{c.label()}",
                        selected: c == form.choice,
                        "{c.label()}"
                    }
                }
            }
        }
        if form.choice == IndexChoice::Vector {
            div { class: "field",
                label { r#for: "ix-metric", "Metric" }
                select {
                    id: "ix-metric",
                    class: "select native",
                    disabled,
                    onchange: move |e| {
                        let v = metric_of(&e.value());
                        e4(Box::new(move |f| f.metric = v));
                    },
                    for m in VectorMetric::ALL {
                        option {
                            key: "{m.ddl()}",
                            value: "{m.ddl()}",
                            selected: m == form.metric,
                            "{m.ddl()}"
                        }
                    }
                }
            }
        }
    }
}

fn preview(form: &IndexForm, schema: &DesignSchema) -> Vec<String> {
    designer_ddl::index_statement(&form.to_index(schema)).unwrap_or_default()
}

/// The New index form.
#[component]
pub fn NewIndexForm(form: IndexForm, schema: DesignSchema) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let mut state = use_context::<Signal<DesignerState>>();
    let issues = form.issues(&schema);
    let error = state.read().form_error.clone();
    let lines = preview(&form, &schema);
    let stage = {
        let (form, schema) = (form.clone(), schema.clone());
        move |_| {
            let (backend, index) = (backend.clone(), form.to_index(&schema));
            spawn_forever(async move { tasks::stage_new_index(state, backend, index).await });
        }
    };
    rsx! {
        div { class: "inspector insp-form",
            div { class: "insp-head",
                div { class: "insp-title", h3 { "New index" } }
                p { class: "insp-sub", "Staged first. The database changes only after review." }
            }
            IndexFields {
                form: form.clone(),
                schema: schema.clone(),
                disabled: false,
                on_name: {
                    let form = form.clone();
                    move |name: String| {
                        let mut next = form.clone();
                        next.name = name;
                        state.write().inspector = Inspector::NewIndex(next);
                    }
                },
                on_change: move |next| state.write().inspector = Inspector::NewIndex(next),
            }
            for message in issues.iter() {
                div { key: "{message}", class: "field-error", "{message}" }
            }
            if let Some(e) = error {
                div { class: "field-error", role: "alert", "{e}" }
            }
            div { class: "insp-label", "DDL preview" }
            Ddl { lines }
            div { class: "inspector-actions",
                button {
                    class: "btn primary",
                    disabled: !issues.is_empty(),
                    onclick: stage,
                    "Stage index"
                }
                button {
                    class: "btn",
                    onclick: move |_| {
                        let mut s = state.write();
                        s.inspector = Inspector::Selection;
                        s.form_error = None;
                    },
                    "Cancel"
                }
            }
        }
    }
}

/// The New reference form: workspace metadata, not a constraint.
#[component]
pub fn NewReferenceForm(form: ReferenceForm, schema: DesignSchema) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let mut state = use_context::<Signal<DesignerState>>();
    let issues = form.issues(&schema);
    let error = state.read().form_error.clone();
    let fields_of = |c: &str| -> Vec<String> {
        schema
            .collection(c)
            .map(|c| c.fields.iter().map(|f| f.name().to_string()).collect())
            .unwrap_or_default()
    };
    let (from_fields, to_fields) = (
        fields_of(&form.from_collection),
        fields_of(&form.to_collection),
    );
    let edit = {
        let (form, schema) = (form.clone(), schema.clone());
        move |change: Box<dyn FnOnce(&mut ReferenceForm)>| {
            let mut next = form.clone();
            change(&mut next);
            next.settle(&schema);
            state.write().inspector = Inspector::NewReference(next);
        }
    };
    let (mut e1, mut e2, mut e3, mut e4) = (edit.clone(), edit.clone(), edit.clone(), edit);
    let stage = {
        let form = form.clone();
        move |_| {
            let (backend, reference) = (backend.clone(), form.to_reference());
            spawn_forever(async move { tasks::stage_reference(state, backend, reference).await });
        }
    };
    rsx! {
        div { class: "inspector insp-form",
            div { class: "insp-head",
                div { class: "insp-title", h3 { "New logical reference" } }
                p { class: "insp-sub",
                    "A design annotation. NodeDB does not enforce it, and no DDL runs."
                }
            }
            div { class: "field",
                label { r#for: "rf-from-c", "From collection" }
                select {
                    id: "rf-from-c",
                    class: "select native",
                    onchange: move |e| {
                        let v = e.value();
                        e1(Box::new(move |f| f.from_collection = v));
                    },
                    for c in schema.collections.iter() {
                        option { key: "{c.name()}", value: "{c.name()}", selected: c.name() == form.from_collection, "{c.name()}" }
                    }
                }
            }
            div { class: "field",
                label { r#for: "rf-from-f", "Field that holds it" }
                select {
                    id: "rf-from-f",
                    class: "select native",
                    onchange: move |e| {
                        let v = e.value();
                        e2(Box::new(move |f| f.from_field = v));
                    },
                    for f in from_fields.iter() {
                        option { key: "{f}", value: "{f}", selected: *f == form.from_field, "{f}" }
                    }
                }
            }
            div { class: "field",
                label { r#for: "rf-to-c", "References collection" }
                select {
                    id: "rf-to-c",
                    class: "select native",
                    onchange: move |e| {
                        let v = e.value();
                        e3(Box::new(move |f| f.to_collection = v));
                    },
                    for c in schema.collections.iter() {
                        option { key: "{c.name()}", value: "{c.name()}", selected: c.name() == form.to_collection, "{c.name()}" }
                    }
                }
            }
            div { class: "field",
                label { r#for: "rf-to-f", "References field" }
                select {
                    id: "rf-to-f",
                    class: "select native",
                    onchange: move |e| {
                        let v = e.value();
                        e4(Box::new(move |f| f.to_field = v));
                    },
                    for f in to_fields.iter() {
                        option { key: "{f}", value: "{f}", selected: *f == form.to_field, "{f}" }
                    }
                }
            }
            for message in issues.iter() {
                div { key: "{message}", class: "field-error", "{message}" }
            }
            if let Some(e) = error {
                div { class: "field-error", role: "alert", "{e}" }
            }
            div { class: "inspector-actions",
                button {
                    class: "btn primary",
                    disabled: !issues.is_empty(),
                    onclick: stage,
                    "Stage reference"
                }
                button {
                    class: "btn",
                    onclick: move |_| {
                        let mut s = state.write();
                        s.inspector = Inspector::Selection;
                        s.form_error = None;
                    },
                    "Cancel"
                }
            }
        }
    }
}

/// A selected index. A staged one edits in place. An applied one reads.
#[component]
pub fn IndexInspector(name: String, schema: DesignSchema, pending: Vec<PendingChange>) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let state = use_context::<Signal<DesignerState>>();
    let Some(index) = schema.index(&name).cloned() else {
        return rsx! { div { class: "async-empty", "`{name}` is no longer in the schema." } };
    };
    let change_id = change_of_index(&pending, &name).map(|c| c.id.clone());
    let (tab, error) = {
        let s = state.read();
        (s.tab, s.form_error.clone())
    };
    let form = IndexForm::of(&index, change_id.as_deref().unwrap_or_default());
    let editable = change_id.is_some();
    let lines = preview(&form, &schema);
    let kind = index
        .choice()
        .map(|c| format!("{} index", c.label()))
        .unwrap_or_else(|| "Index".to_string());
    // Drafts never list their own name as a clash: the server compares
    // against every other staged change.
    let restage = {
        let (backend, schema) = (backend.clone(), schema.clone());
        move |next: IndexForm| {
            let (backend, index) = (backend.clone(), next.to_index(&schema));
            let next_form = next.clone();
            spawn_forever(
                async move { tasks::restage_index(state, backend, next_form, index).await },
            );
        }
    };
    let rename = {
        let (restage, form) = (restage.clone(), form.clone());
        move |name: String| {
            let mut next = form.clone();
            next.name = name;
            restage(next);
        }
    };
    let discard = {
        let backend = backend.clone();
        let id = change_id.clone().unwrap_or_default();
        move |_| {
            let (backend, id) = (backend.clone(), id.clone());
            spawn_forever(async move { tasks::discard(state, backend, id).await });
        }
    };
    rsx! {
        div { class: "inspector",
            InspectorHead {
                icon: index_icon(&index),
                kind,
                title: index.name.clone(),
                state: index.state,
            }
            InspectorTabs {}
            match tab {
                InspectorTab::Definition => rsx! {
                    div { class: "insp-form",
                        IndexFields {
                            form: form.clone(),
                            schema: schema.clone(),
                            disabled: !editable,
                            on_name: rename,
                            on_change: restage,
                        }
                        if let Some(e) = error {
                            div { class: "field-error", role: "alert", "{e}" }
                        }
                        div { class: "insp-label", "DDL preview" }
                        Ddl { lines }
                    }
                },
                InspectorTab::Ddl => rsx! {
                    div { class: "insp-form",
                        Ddl { lines }
                        if !editable {
                            p { class: "insp-sub",
                                "Generated from the catalog. The server does not store the statement."
                            }
                        }
                    }
                },
            }
            div { class: "inspector-note insp-note",
                if editable {
                    span { class: "insp-status",
                        crate::components::icon::Ico { icon: crate::components::icon::Icon::Info, size: 16 }
                        "Not applied to database"
                    }
                    button { class: "link-btn", onclick: discard, "Discard draft" }
                } else {
                    span { class: "insp-status",
                        crate::components::icon::Ico { icon: crate::components::icon::Icon::CircleCheck, size: 16 }
                        "Applied to database"
                    }
                    button {
                        class: "link-btn",
                        disabled: true,
                        title: "Dropping an index has no designed flow yet",
                        "Drop index"
                    }
                }
            }
        }
    }
}
