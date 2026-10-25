//! The statement's `:name` parameters: a type and a value for each. The
//! rows follow the SQL, and a value that does not fit its type says so.

use dioxus::prelude::*;

use crate::models::workbench::{ParamBinding, ParamType};
use crate::state::query::QueryWorkspace;

/// The message under a value that is set but does not fit its type.
fn problem(p: &ParamBinding) -> Option<String> {
    if p.value.trim().is_empty() {
        return None;
    }
    p.check().err().map(|e| e.to_string())
}

#[component]
pub fn ParamsRow() -> Element {
    let mut ws = use_context::<Signal<QueryWorkspace>>();
    let params: Vec<ParamBinding> = ws
        .read()
        .active()
        .map(|t| t.params.clone())
        .unwrap_or_default();
    if params.is_empty() {
        return rsx! {};
    }
    rsx! {
        div { class: "params", role: "group", "aria-label": "Parameters",
            div { class: "field-label params-label", "Parameters" }
            for (i, p) in params.into_iter().enumerate() {
                {
                    let error = problem(&p);
                    rsx! {
                        div { key: "{p.name}", class: "param-row",
                            input {
                                class: "input mono param-name",
                                readonly: true,
                                "aria-label": "Parameter name",
                                value: ":{p.name}",
                            }
                            select {
                                class: "select native compact param-type",
                                "aria-label": "Type of :{p.name}",
                                onchange: move |e| {
                                    let ty = ParamType::ALL
                                        .iter()
                                        .copied()
                                        .find(|t| t.label() == e.value());
                                    if let (Some(ty), Some(tab)) = (ty, ws.write().active_mut())
                                        && let Some(p) = tab.params.get_mut(i)
                                    {
                                        p.ty = ty;
                                    }
                                },
                                for t in ParamType::ALL {
                                    option { key: "{t.label()}", value: "{t.label()}", selected: t == p.ty, "{t.label()}" }
                                }
                            }
                            input {
                                class: if error.is_some() { "input param-value invalid" } else { "input param-value" },
                                "aria-label": "Value of :{p.name}",
                                "aria-invalid": "{error.is_some()}",
                                placeholder: "Value",
                                value: "{p.value}",
                                oninput: move |e| {
                                    if let Some(tab) = ws.write().active_mut()
                                        && let Some(p) = tab.params.get_mut(i)
                                    {
                                        p.value = e.value();
                                    }
                                },
                            }
                        }
                        if let Some(msg) = error.as_ref() {
                            div { class: "field-error", "{msg}" }
                        }
                    }
                }
            }
        }
    }
}
