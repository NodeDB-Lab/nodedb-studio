//! The field table of the Create collection drawer.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::designer_new::{FIELD_TYPES, Issue, NewCollection, Storage, Target as IssueAt};
use crate::models::schema::FieldType;
use crate::state::designer::DesignerState;
use crate::views::designer::create::{edit, issue_at};

fn type_of(label: &str) -> FieldType {
    FIELD_TYPES
        .into_iter()
        .find(|t| t.keyword().eq_ignore_ascii_case(label))
        .unwrap_or(FieldType::String { max: None })
}

#[component]
pub fn FieldsEditor(form: NewCollection, issues: Vec<Issue>, show: bool) -> Element {
    let state = use_context::<Signal<DesignerState>>();
    let table_issue = issue_at(&issues, &IssueAt::Fields, show).map(str::to_string);
    let key_word = match form.storage {
        Storage::KeyValue => "Key",
        Storage::Document | Storage::Columnar => "Primary",
    };
    let heading = if form.schema_choice()
        && form.schema == crate::models::designer_new::SchemaMode::Flexible
    {
        "Declared fields"
    } else {
        "Fields"
    };
    rsx! {
        section { class: "create-fields",
            div { class: "create-fields-head",
                h3 { "{heading}" }
                button {
                    class: "btn sm",
                    onclick: move |_| {
                        edit(state, |c| {
                            let n = c.form.fields.len() + 1;
                            c.form.add_field(&format!("field_{n}"), FieldType::String { max: None }, false, false);
                        });
                    },
                    Ico { icon: Icon::Plus, size: 16 }
                    "Add field"
                }
            }
            if let Some(m) = table_issue {
                div { class: "field-error", role: "alert", "{m}" }
            }
            div { class: "field-table", role: "table", "aria-label": "Fields",
                div { class: "field-table-head", role: "row",
                    span { role: "columnheader", "Name" }
                    span { role: "columnheader", "Type" }
                    span { role: "columnheader", class: "center", "Required" }
                    span { role: "columnheader", class: "center", "{key_word}" }
                    span { role: "columnheader", class: "center", "Remove" }
                }
                for f in form.fields.iter() {
                    {
                        let id = f.id;
                        let issue = issue_at(&issues, &IssueAt::Field(id), show || !f.name.is_empty()).map(str::to_string);
                        rsx! {
                            div { key: "{id}", class: "field-table-row", role: "row",
                                div { class: "cell-stack",
                                    input {
                                        class: if issue.is_some() { "input mono invalid" } else { "input mono" },
                                        value: "{f.name}",
                                        "aria-label": "Field name",
                                        "aria-invalid": "{issue.is_some()}",
                                        spellcheck: "false",
                                        oninput: move |e| edit(state, |c| {
                                            if let Some(x) = c.form.fields.iter_mut().find(|x| x.id == id) {
                                                x.name = e.value();
                                            }
                                        }),
                                    }
                                    if let Some(m) = issue.as_ref() {
                                        span { class: "field-error", "{m}" }
                                    }
                                }
                                select {
                                    class: "select native compact",
                                    "aria-label": "Field type",
                                    onchange: move |e| edit(state, |c| {
                                        if let Some(x) = c.form.fields.iter_mut().find(|x| x.id == id) {
                                            x.ty = type_of(&e.value());
                                        }
                                    }),
                                    for t in FIELD_TYPES {
                                        option {
                                            key: "{t.keyword()}",
                                            value: "{t.keyword()}",
                                            selected: t == f.ty,
                                            "{t.keyword().to_lowercase()}"
                                        }
                                    }
                                }
                                span { class: "center",
                                    input {
                                        r#type: "checkbox",
                                        class: "checkbox",
                                        "aria-label": "Required",
                                        checked: f.required || f.primary,
                                        disabled: f.primary,
                                        title: if f.primary { "A primary field is always required" } else { "" },
                                        onchange: move |e| edit(state, |c| {
                                            if let Some(x) = c.form.fields.iter_mut().find(|x| x.id == id) {
                                                x.required = e.checked();
                                            }
                                        }),
                                    }
                                }
                                span { class: "center",
                                    input {
                                        r#type: "checkbox",
                                        class: "checkbox",
                                        "aria-label": "{key_word} field",
                                        checked: f.primary,
                                        onchange: move |e| edit(state, |c| {
                                            let on = e.checked();
                                            for x in c.form.fields.iter_mut() {
                                                x.primary = on && x.id == id;
                                            }
                                        }),
                                    }
                                }
                                span { class: "center",
                                    button {
                                        class: "icon-btn sm",
                                        "aria-label": "Remove field {f.name}",
                                        onclick: move |_| edit(state, |c| c.form.fields.retain(|x| x.id != id)),
                                        Ico { icon: Icon::Trash, size: 16 }
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
