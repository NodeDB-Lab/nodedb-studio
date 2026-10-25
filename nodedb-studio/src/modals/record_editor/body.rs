//! The editing surface: the JSON editor and the typed form over it.

use dioxus::prelude::*;
use nodedb_types::Value;

use crate::models::record_check::{input_text, set_field};
use crate::models::record_json::parse_object;
use crate::models::schema::FieldType;
use crate::state::record_editor::{EditorMode, EditorTab, RecordEditor, Stage};

/// The JSON text with line numbers and colors. The textarea sits over a
/// colored copy of the same text, so the colors follow every keystroke.
#[component]
pub fn JsonEditor(text: String, locked: bool, on_input: EventHandler<String>) -> Element {
    let lines = text.lines().count().max(1);
    rsx! {
        div { class: "json-editor",
            div { class: "gutter", "aria-hidden": "true",
                for n in 1..=lines {
                    div { key: "{n}", "{n}" }
                }
            }
            div { class: "editor-stack",
                pre { class: "highlight", "aria-hidden": "true",
                    for (i, line) in text.lines().enumerate() {
                        div { key: "{i}",
                            for (j, (kind, part)) in crate::models::record_json::tokens(line).into_iter().enumerate() {
                                span { key: "{j}", class: "{kind.class()}", "{part}" }
                            }
                            if line.is_empty() { " " }
                        }
                    }
                }
                textarea {
                    class: "input-layer",
                    spellcheck: false,
                    wrap: "off",
                    readonly: locked,
                    "aria-label": "Record JSON",
                    value: "{text}",
                    oninput: move |e| on_input.call(e.value()),
                }
            }
        }
    }
}

/// One form control for a field.
#[component]
fn FieldControl(
    name: String,
    ty: FieldType,
    value: String,
    locked: bool,
    on_input: EventHandler<String>,
) -> Element {
    match ty {
        FieldType::Bool => rsx! {
            input {
                id: "fld-{name}",
                r#type: "checkbox",
                class: "checkbox",
                checked: value == "true",
                disabled: locked,
                onchange: move |e| on_input.call(if e.checked() { "true".to_string() } else { "false".to_string() }),
            }
        },
        FieldType::Object | FieldType::Array | FieldType::Json | FieldType::Geometry { .. } => {
            rsx! {
                textarea {
                    id: "fld-{name}",
                    class: "textarea mono",
                    rows: "4",
                    readonly: locked,
                    value: "{value}",
                    onchange: move |e| on_input.call(e.value()),
                }
            }
        }
        FieldType::String { .. }
        | FieldType::Int64
        | FieldType::Float64
        | FieldType::Decimal { .. }
        | FieldType::Timestamp
        | FieldType::Datetime
        | FieldType::Vector { .. }
        | FieldType::Bytes => rsx! {
            input {
                id: "fld-{name}",
                class: if ty.numeric() { "input mono" } else { "input" },
                inputmode: if ty.numeric() { "decimal" } else { "text" },
                readonly: locked,
                value: "{value}",
                oninput: move |e| on_input.call(e.value()),
            }
        },
    }
}

#[component]
pub fn FormEditor(editor: Signal<Option<RecordEditor>>) -> Element {
    let Some(e) = editor.read().clone() else {
        return rsx! {};
    };
    let locked = e.busy || e.stage == Stage::Reviewing;
    let parsed = parse_object(&e.text, &e.schema.fields);
    let Ok(Value::Object(map)) = parsed else {
        return rsx! {
            div { class: "callout warn",
                "The JSON does not parse, so the form cannot show it. Fix the JSON tab first."
            }
        };
    };
    let id_field = e.schema.id_field.clone();
    rsx! {
        div { class: "form-fields",
            for f in e.schema.fields.iter().filter(|f| !matches!(f.ty, FieldType::Vector { .. })) {
                {
                    let name = f.name.clone();
                    let value = input_text(map.get(&f.name), &f.ty);
                    // The id is fixed once the record exists.
                    let fixed = e.mode == EditorMode::Edit && id_field.as_deref() == Some(f.name.as_str());
                    rsx! {
                        div { key: "{f.name}", class: "form-line",
                            label { r#for: "fld-{f.name}",
                                "{f.name}"
                                if f.required { span { class: "required", " *" } }
                                span { class: "field-hint", "{f.ty.label()}" }
                            }
                            FieldControl {
                                name: f.name.clone(),
                                ty: f.ty,
                                value,
                                locked: locked || fixed,
                                on_input: move |text: String| {
                                    let current = editor.peek().as_ref().map(|e| (e.text.clone(), e.schema.fields.clone()));
                                    if let Some((t, fields)) = current
                                        && let Ok(next) = set_field(&t, &fields, &name, &text)
                                        && let Some(ed) = editor.write().as_mut()
                                    {
                                        ed.text = next;
                                    }
                                },
                            }
                        }
                    }
                }
            }
            p { class: "meta", "Fields outside the schema, and embeddings, are edited in the JSON tab or kept as stored." }
        }
    }
}

/// The tab strip: Form and JSON.
#[component]
pub fn EditorTabs(editor: Signal<Option<RecordEditor>>, tab: EditorTab) -> Element {
    let items = ["Form", "JSON"];
    let active = usize::from(tab == EditorTab::Json);
    rsx! {
        crate::components::tabs::TabBar {
            items: items.to_vec(),
            active,
            class: "editor-tabs",
            on_select: move |i: usize| {
                if let Some(ed) = editor.write().as_mut() {
                    ed.tab = if i == 0 { EditorTab::Form } else { EditorTab::Json };
                }
            },
        }
    }
}
