//! The Create collection drawer: name, storage, fields, constraints, and
//! capabilities. Review DDL stages the collection and opens the review.
//! Nothing reaches the database from here.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::designer::DesignSchema;
use crate::models::designer_new::{SchemaMode, Storage};
use crate::models::designer_new_check::{Issue, Target as IssueAt};
use crate::services::backend::Backend;
use crate::state::designer::{CreateState, DesignerState};
use crate::views::designer::create_fields::FieldsEditor;
use crate::views::designer::create_side::{Capabilities, Constraints};
use crate::views::designer::tasks;

/// Change the form under edit.
pub fn edit(mut state: Signal<DesignerState>, change: impl FnOnce(&mut CreateState)) {
    if let Some(c) = state.write().create.as_mut() {
        change(c);
        c.form.settle_capability_fields();
    }
}

/// The first issue at `at`, once the form has been tried or the user typed.
pub fn issue_at<'a>(issues: &'a [Issue], at: &IssueAt, show: bool) -> Option<&'a str> {
    show.then(|| {
        issues
            .iter()
            .find(|i| i.at == *at)
            .map(|i| i.message.as_str())
    })
    .flatten()
}

#[component]
pub fn CreateDrawer(schema: DesignSchema, create: CreateState) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let mut state = use_context::<Signal<DesignerState>>();
    let issues = create.form.issues(&schema);
    let form = create.form.clone();
    let show_name = create.tried || !form.name.is_empty();
    let name_issue = issue_at(&issues, &IssueAt::Name, show_name).map(str::to_string);
    let blocked = create.tried && !issues.is_empty();
    let review = {
        let (form, schema) = (form.clone(), schema.clone());
        move |_| {
            let issues = form.issues(&schema);
            if !issues.is_empty() {
                edit(state, |c| c.tried = true);
                return;
            }
            let (backend, kinds, name) = (
                backend.clone(),
                form.clone().into_changes(),
                form.name.clone(),
            );
            spawn_forever(
                async move { tasks::create_collection(state, backend, kinds, name).await },
            );
        }
    };
    rsx! {
        aside { class: "sheet create-sheet", "aria-label": "Create collection",
            header { class: "panel-header",
                span { class: "panel-grip", "aria-hidden": "true",
                    Ico { icon: Icon::GripVertical, size: 16 }
                }
                h2 { class: "panel-title", "Create collection" }
                div { class: "panel-actions" }
                button {
                    class: "icon-btn panel-close",
                    "aria-label": "Close Create collection",
                    onclick: move |_| state.write().create = None,
                    Ico { icon: Icon::X, size: 16 }
                }
            }
            div { class: "sheet-body",
                div { class: "create-main",
                    div { class: "form-row",
                        label { r#for: "cc-name", "Name" }
                        div { class: "form-control",
                            input {
                                id: "cc-name",
                                class: if name_issue.is_some() { "input invalid" } else { "input" },
                                value: "{form.name}",
                                placeholder: "shipments",
                                spellcheck: "false",
                                "aria-invalid": "{name_issue.is_some()}",
                                oninput: move |e| edit(state, |c| c.form.name = e.value()),
                            }
                            if let Some(m) = name_issue.as_ref() {
                                div { class: "field-error", role: "alert", "{m}" }
                            }
                        }
                    }
                    div { class: "form-row",
                        label { r#for: "cc-db", "Database" }
                        div { class: "form-control",
                            select {
                                id: "cc-db",
                                class: "select native",
                                disabled: true,
                                title: "A new collection is created in the connected database",
                                option { value: "{form.database}", selected: true, "{form.database}" }
                            }
                        }
                    }
                    div { class: "form-row",
                        span { class: "form-label", id: "cc-storage", "Storage" }
                        div {
                            class: "choice-group",
                            role: "radiogroup",
                            "aria-labelledby": "cc-storage",
                            for s in Storage::ALL {
                                label {
                                    key: "{s.label()}",
                                    class: if form.storage == s { "choice selected" } else { "choice" },
                                    input {
                                        r#type: "radio",
                                        class: "radio",
                                        name: "cc-storage",
                                        checked: form.storage == s,
                                        onchange: move |_| {
                                            edit(state, |c| {
                                                c.form.storage = s;
                                                c.form.settle_schema();
                                            });
                                        },
                                    }
                                    "{s.label()}"
                                }
                            }
                        }
                    }
                    div { class: "form-row",
                        span { class: "form-label", id: "cc-schema", "Schema" }
                        div {
                            class: "choice-group",
                            role: "radiogroup",
                            "aria-labelledby": "cc-schema",
                            for m in SchemaMode::ALL {
                                {
                                    let locked = !form.schema_choice();
                                    rsx! {
                                        label {
                                            key: "{m.label()}",
                                            class: if form.schema == m { "choice selected" } else { "choice" },
                                            title: if locked { "This storage has one schema mode" } else { "" },
                                            input {
                                                r#type: "radio",
                                                class: "radio",
                                                name: "cc-schema",
                                                checked: form.schema == m,
                                                disabled: locked,
                                                onchange: move |_| edit(state, |c| c.form.schema = m),
                                            }
                                            "{m.label()}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                    FieldsEditor { form: form.clone(), issues: issues.clone(), show: create.tried }
                }
                div { class: "create-side",
                    Constraints {
                        form: form.clone(),
                        schema: schema.clone(),
                        issues: issues.clone(),
                        open: create.open,
                        show: create.tried,
                    }
                    Capabilities { form: form.clone(), issues: issues.clone(), show: create.tried }
                }
            }
            footer { class: "sheet-footer",
                if let Some(e) = create.error.clone() {
                    div { class: "field-error", role: "alert", "{e}" }
                } else if blocked {
                    div { class: "field-error", role: "alert",
                        "Fix {issues.len()} issue(s) above to review the DDL."
                    }
                } else {
                    span { class: "sheet-hint", "Nothing is created until you review and apply the DDL." }
                }
                div { class: "toolbar-spacer" }
                button {
                    class: "btn primary",
                    disabled: create.busy,
                    onclick: review,
                    "Review DDL"
                }
                button { class: "btn", onclick: move |_| state.write().create = None, "Cancel" }
            }
        }
    }
}
