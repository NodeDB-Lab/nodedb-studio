//! Constraints and capabilities in the Create collection drawer.
//!
//! A foreign key here is a logical reference: NodeDB does not enforce it,
//! so staging it records workspace metadata and generates no DDL.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::collection::Attached;
use crate::models::designer::DesignSchema;
use crate::models::designer_new::{Constraint, ConstraintKind, NewCollection};
use crate::models::designer_new_check::{Issue, Target as IssueAt};
use crate::models::schema::FieldType;
use crate::state::designer::DesignerState;
use crate::views::designer::create::{edit, issue_at};

fn kind_icon(kind: ConstraintKind) -> Icon {
    match kind {
        ConstraintKind::Unique => Icon::Key,
        ConstraintKind::ForeignKey => Icon::Link,
        ConstraintKind::Check => Icon::Braces,
    }
}

fn fields_of(schema: &DesignSchema, collection: &str) -> Vec<String> {
    schema
        .collection(collection)
        .map(|c| c.fields.iter().map(|f| f.name().to_string()).collect())
        .unwrap_or_default()
}

/// One constraint's editor.
#[component]
fn ConstraintEditor(
    constraint: Constraint,
    form: NewCollection,
    schema: DesignSchema,
    issue: Option<String>,
) -> Element {
    let state = use_context::<Signal<DesignerState>>();
    let id = constraint.id();
    let own: Vec<String> = form
        .fields
        .iter()
        .filter(|f| !f.name.is_empty())
        .map(|f| f.name.clone())
        .collect();
    let body = match &constraint {
        Constraint::Unique { fields, .. } => rsx! {
            div { class: "field",
                span { class: "field-label", "Fields that must be unique together" }
                div { class: "check-list",
                    for name in own.iter() {
                        {
                            let name = name.clone();
                            let on = fields.contains(&name);
                            rsx! {
                                label { key: "{name}", class: "check-row",
                                    input {
                                        r#type: "checkbox",
                                        class: "checkbox",
                                        checked: on,
                                        onchange: {
                                            let name = name.clone();
                                            move |e| edit(state, |c| {
                                                if let Some(Constraint::Unique { fields, .. }) =
                                                    c.form.constraints.iter_mut().find(|k| k.id() == id)
                                                {
                                                    fields.retain(|f| *f != name);
                                                    if e.checked() {
                                                        fields.push(name.clone());
                                                    }
                                                }
                                            })
                                        },
                                    }
                                    span { class: "mono", "{name}" }
                                }
                            }
                        }
                    }
                }
            }
        },
        Constraint::ForeignKey {
            field,
            to_collection,
            to_field,
            ..
        } => {
            let targets = fields_of(&schema, to_collection);
            rsx! {
                div { class: "form-row tight",
                    label { r#for: "fk-field-{id}", "Field" }
                    select {
                        id: "fk-field-{id}",
                        class: "select native",
                        onchange: move |e| edit(state, |c| {
                            if let Some(Constraint::ForeignKey { field, .. }) =
                                c.form.constraints.iter_mut().find(|k| k.id() == id)
                            {
                                *field = e.value();
                            }
                        }),
                        option { value: "", selected: field.is_empty(), "Select a field" }
                        for f in own.iter() {
                            option { key: "{f}", value: "{f}", selected: f == field, "{f}" }
                        }
                    }
                }
                div { class: "form-row tight",
                    label { r#for: "fk-coll-{id}", "References" }
                    div { class: "pair",
                        select {
                            id: "fk-coll-{id}",
                            class: "select native",
                            "aria-label": "Referenced collection",
                            onchange: {
                                let schema = schema.clone();
                                move |e| edit(state, |c| {
                                    let v = e.value();
                                    let first = fields_of(&schema, &v).into_iter().next().unwrap_or_default();
                                    if let Some(Constraint::ForeignKey { to_collection, to_field, .. }) =
                                        c.form.constraints.iter_mut().find(|k| k.id() == id)
                                    {
                                        *to_collection = v;
                                        *to_field = first;
                                    }
                                })
                            },
                            option { value: "", selected: to_collection.is_empty(), "Select a collection" }
                            for c in schema.collections.iter().filter(|c| c.name() != form.name) {
                                option { key: "{c.name()}", value: "{c.name()}", selected: c.name() == to_collection, "{c.name()}" }
                            }
                        }
                        select {
                            class: "select native",
                            "aria-label": "Referenced field",
                            disabled: targets.is_empty(),
                            onchange: move |e| edit(state, |c| {
                                if let Some(Constraint::ForeignKey { to_field, .. }) =
                                    c.form.constraints.iter_mut().find(|k| k.id() == id)
                                {
                                    *to_field = e.value();
                                }
                            }),
                            for f in targets.iter() {
                                option { key: "{f}", value: "{f}", selected: f == to_field, "{f}" }
                            }
                        }
                    }
                }
                p { class: "insp-sub",
                    "A logical reference. NodeDB does not enforce it, so it is saved in the workspace and runs no DDL."
                }
            }
        }
        Constraint::Check { expression, .. } => rsx! {
            div { class: "field",
                label { r#for: "ck-{id}", "Condition a record must meet" }
                input {
                    id: "ck-{id}",
                    class: "input mono",
                    value: "{expression}",
                    placeholder: "status <> ''",
                    spellcheck: "false",
                    oninput: move |e| edit(state, |c| {
                        if let Some(Constraint::Check { expression, .. }) =
                            c.form.constraints.iter_mut().find(|k| k.id() == id)
                        {
                            *expression = e.value();
                        }
                    }),
                }
            }
        },
    };
    rsx! {
        div { class: "constraint",
            {body}
            if let Some(m) = issue {
                div { class: "field-error", role: "alert", "{m}" }
            }
            button {
                class: "link-btn",
                onclick: move |_| edit(state, |c| c.form.constraints.retain(|k| k.id() != id)),
                Ico { icon: Icon::Trash, size: 16 }
                "Remove constraint"
            }
        }
    }
}

#[component]
pub fn Constraints(
    form: NewCollection,
    schema: DesignSchema,
    issues: Vec<Issue>,
    open: Option<ConstraintKind>,
    show: bool,
) -> Element {
    let state = use_context::<Signal<DesignerState>>();
    let mut menu = use_signal(|| false);
    rsx! {
        section { class: "create-section",
            div { class: "create-section-head",
                h3 { "Constraints" }
                div { class: "anchor",
                    button {
                        class: "btn sm",
                        "aria-haspopup": "menu",
                        onclick: move |_| menu.toggle(),
                        Ico { icon: Icon::Plus, size: 16 }
                        "Add constraint"
                    }
                    if menu() {
                        div { class: "popover align-right", role: "menu",
                            for k in ConstraintKind::ALL {
                                button {
                                    key: "{k.label()}",
                                    class: "menu-item",
                                    role: "menuitem",
                                    onclick: move |_| {
                                        menu.set(false);
                                        edit(state, |c| {
                                            c.form.add_constraint(k);
                                            c.open = Some(k);
                                        });
                                    },
                                    Ico { icon: kind_icon(k), size: 16 }
                                    "{k.label()}"
                                }
                            }
                        }
                    }
                }
            }
            div { class: "group-list",
                for k in ConstraintKind::ALL {
                    {
                        let items: Vec<Constraint> = form
                            .constraints
                            .iter()
                            .filter(|c| c.kind() == k)
                            .cloned()
                            .collect();
                        let is_open = open == Some(k);
                        rsx! {
                            div { key: "{k.label()}", class: if is_open { "group open" } else { "group" },
                                button {
                                    class: "group-head",
                                    "aria-expanded": "{is_open}",
                                    onclick: move |_| edit(state, |c| {
                                        c.open = if c.open == Some(k) { None } else { Some(k) };
                                    }),
                                    Ico { icon: kind_icon(k), size: 16 }
                                    span { class: "group-name", "{k.label()}" }
                                    if !items.is_empty() {
                                        span { class: "count-pill", "{items.len()}" }
                                    }
                                    Ico { icon: if is_open { Icon::ChevronDown } else { Icon::ChevronRight }, size: 16 }
                                }
                                if is_open {
                                    div { class: "group-body",
                                        for c in items.iter() {
                                            ConstraintEditor {
                                                key: "{c.id()}",
                                                constraint: c.clone(),
                                                form: form.clone(),
                                                schema: schema.clone(),
                                                issue: issue_at(&issues, &IssueAt::Constraint(c.id()), show).map(str::to_string),
                                            }
                                        }
                                        if items.is_empty() {
                                            p { class: "insp-sub", "No {k.label().to_lowercase()} constraint yet." }
                                        }
                                        if items.is_empty() {
                                            button {
                                                class: "btn sm",
                                                onclick: move |_| edit(state, |c| c.form.add_constraint(k)),
                                                Ico { icon: Icon::Plus, size: 16 }
                                                "Add {k.label().to_lowercase()}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Attachments: capabilities that work over the collection's records.
#[component]
pub fn Capabilities(form: NewCollection, issues: Vec<Issue>, show: bool) -> Element {
    let state = use_context::<Signal<DesignerState>>();
    let documents = form.profile() == crate::models::collection::Profile::Document
        || form.profile() == crate::models::collection::Profile::DocumentStrict;
    let text = form.names_of(|t| matches!(t, FieldType::String { .. }));
    let vector = form.names_of(|t| matches!(t, FieldType::Vector { .. }));
    rsx! {
        section { class: "create-section",
            div { class: "create-section-head",
                h3 { "Capabilities" }
                span {
                    class: "info-tip",
                    title: "Attachments work over this collection's records. They are not storage types.",
                    Ico { icon: Icon::Info, size: 16 }
                }
            }
            div { class: "group-list",
                for (cap , label , icon) in [
                    (Attached::Graph, "Graph", Icon::Network),
                    (Attached::FullText, "Full-text", Icon::FileText),
                    (Attached::Vector, "Vector", Icon::Grid3x3),
                ] {
                    {
                        let on = form.attached(cap);
                        let locked = !documents;
                        let choices = match cap {
                            Attached::FullText => text.clone(),
                            Attached::Vector => vector.clone(),
                            Attached::Graph => Vec::new(),
                        };
                        let current = match cap {
                            Attached::FullText => form.text_field.clone(),
                            Attached::Vector => form.vector_field.clone(),
                            Attached::Graph => String::new(),
                        };
                        let issue = issue_at(&issues, &IssueAt::Capability(cap), show || on).map(str::to_string);
                        rsx! {
                            div { key: "{label}", class: "capability",
                                label { class: "capability-row",
                                    Ico { icon, size: 16 }
                                    span { class: "group-name", "{label}" }
                                    input {
                                        r#type: "checkbox",
                                        class: "switch",
                                        role: "switch",
                                        "aria-label": "Attach {label}",
                                        checked: on,
                                        disabled: locked,
                                        title: if locked { "Capabilities attach to document collections" } else { "" },
                                        onchange: move |e| edit(state, |c| c.form.set_attached(cap, e.checked())),
                                    }
                                }
                                if on {
                                    div { class: "capability-detail",
                                        match cap {
                                            Attached::Graph => rsx! {
                                                span { class: "insp-sub", "Edges link records. Add them with GRAPH INSERT EDGE. No DDL runs." }
                                            },
                                            Attached::FullText | Attached::Vector => rsx! {
                                                label { class: "toolbar-label", r#for: "cap-{label}", "Field" }
                                                select {
                                                    id: "cap-{label}",
                                                    class: "select native compact",
                                                    disabled: choices.is_empty(),
                                                    onchange: move |e| edit(state, |c| {
                                                        match cap {
                                                            Attached::FullText => c.form.text_field = e.value(),
                                                            Attached::Vector => c.form.vector_field = e.value(),
                                                            Attached::Graph => {}
                                                        }
                                                    }),
                                                    if choices.is_empty() {
                                                        option { value: "", selected: true, "No field fits" }
                                                    }
                                                    for f in choices.iter() {
                                                        option { key: "{f}", value: "{f}", selected: *f == current, "{f}" }
                                                    }
                                                }
                                            },
                                        }
                                        if let Some(m) = issue {
                                            div { class: "field-error", role: "alert", "{m}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
