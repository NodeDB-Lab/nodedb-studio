//! The strip under the query: the fields each mode uses, the vector input,
//! the filters, and the advanced ranking controls.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::records::Filter;
use crate::models::search::{SearchCollection, SearchMode};
use crate::models::search_form::{
    FusionForm, Issue, IssueField, SearchForm, VectorEditor, VectorKind,
};
use crate::state::search::{FilterDraft, SearchUi};
use crate::views::search::query_bar::edit;

#[component]
pub fn QueryStrip(
    collection: SearchCollection,
    form: SearchForm,
    issues: Vec<Issue>,
    provider: Option<String>,
) -> Element {
    let ui = use_context::<SearchUi>();
    let r = ui.results;
    let state = r.read().clone();
    let tried = state.tried;
    let has = |f: IssueField| tried && issues.iter().any(|i| i.field == f);
    let mode = form.mode;
    let hybrid = mode == SearchMode::Hybrid;
    let dims = collection.vector.as_ref().map_or(0, |v| v.dimensions);
    let vector_field = collection.vector.as_ref().map(|v| v.field.clone());
    let can_filter = !collection.filter_fields.is_empty();
    let first_issue = if tried {
        issues.first().map(|i| i.message.clone())
    } else {
        None
    };
    let text_hint = match &provider {
        Some(name) => format!("Text is embedded only by {name}, which this search does not use."),
        None => "No embedding provider is configured, so text is never embedded.".to_string(),
    };

    rsx! {
        div { class: "srch-strip",
            if mode.uses_text() {
                div { class: "srch-ctl",
                    label { r#for: "srch-text-field", "Text field" }
                    select {
                        id: "srch-text-field",
                        class: "select native",
                        onchange: move |e| {
                            let v = e.value();
                            edit(r, |s| s.form.text_field = v);
                        },
                        for f in collection.text_fields().iter() {
                            option { key: "{f}", value: "{f}", selected: *f == form.text_field, "{f}" }
                        }
                    }
                }
            }
            if mode.uses_vector() {
                div { class: "srch-ctl",
                    label { r#for: "srch-vector-field", "Vector field" }
                    select {
                        id: "srch-vector-field",
                        class: "select native",
                        for f in vector_field.iter() {
                            option { key: "{f}", value: "{f}", selected: true, "{f}" }
                        }
                    }
                }
                div { class: "srch-ctl anchor",
                    span { class: "srch-ctl-label", "Vector input" }
                    span {
                        class: if has(IssueField::Vector) { "srch-chip invalid" } else { "srch-chip" },
                        title: "{text_hint}",
                        "{form.vector.label()}"
                    }
                    button {
                        class: "link-btn",
                        r#type: "button",
                        "aria-expanded": "{state.vector_edit.is_some()}",
                        onclick: {
                            let current = form.vector.clone();
                            move |_| edit(r, |s| {
                                s.vector_error = None;
                                s.vector_edit = if s.vector_edit.is_some() { None } else { Some(VectorEditor::of(&current)) };
                            })
                        },
                        "Edit"
                    }
                    if let Some(editor) = state.vector_edit.clone() {
                        VectorPopover {
                            editor,
                            dimensions: dims,
                            error: state.vector_error.clone(),
                            hint: text_hint.clone(),
                        }
                    }
                }
            }
            span { class: "srch-sep" }
            div { class: "srch-filters",
                for f in form.filters.iter() {
                    {
                        let gone = f.clone();
                        rsx! {
                            span { key: "{f.label()}", class: "chip",
                                "{f.label()}"
                                button {
                                    class: "chip-remove",
                                    r#type: "button",
                                    title: "Remove the filter {f.label()}",
                                    "aria-label": "Remove the filter {f.label()}",
                                    onclick: move |_| edit(r, |s| s.form.filters.retain(|x| *x != gone)),
                                    Ico { icon: Icon::X, size: 12 }
                                }
                            }
                        }
                    }
                }
                div { class: "anchor",
                    button {
                        class: "btn",
                        r#type: "button",
                        disabled: !can_filter,
                        title: if can_filter { "Keep only records that match, in every retrieval leg, before the results are cut" } else { "{collection.name} has no field a filter can name" },
                        "aria-expanded": "{state.filter_edit.is_some()}",
                        onclick: {
                            let first = collection.filter_fields.first().cloned().unwrap_or_default();
                            move |_| edit(r, |s| {
                                s.filter_edit = if s.filter_edit.is_some() {
                                    None
                                } else {
                                    Some(FilterDraft { field: first.clone(), value: String::new() })
                                };
                            })
                        },
                        Ico { icon: Icon::Plus, size: 16 }
                        "Filter"
                    }
                    if let Some(draft) = state.filter_edit.clone() {
                        FilterPopover { draft, fields: collection.filter_fields.clone() }
                    }
                }
            }
            div { class: "toolbar-spacer" }
            if hybrid {
                button {
                    class: "link-btn srch-advanced",
                    r#type: "button",
                    "aria-expanded": "{state.advanced}",
                    onclick: move |_| edit(r, |s| s.advanced = !s.advanced),
                    "Advanced ranking"
                    Ico { icon: if state.advanced { Icon::ChevronUp } else { Icon::ChevronDown }, size: 14 }
                }
            }
        }
        if hybrid && state.advanced {
            AdvancedRanking { fusion: form.fusion.clone(), invalid: has(IssueField::Fusion) }
        }
        if let Some(message) = first_issue {
            div { class: "srch-issues", role: "alert", "{message}" }
        }
    }
}

#[component]
fn VectorPopover(
    editor: VectorEditor,
    dimensions: u32,
    error: Option<String>,
    hint: String,
) -> Element {
    let ui = use_context::<SearchUi>();
    let r = ui.results;
    let apply_editor = editor.clone();
    rsx! {
        form {
            class: "popover align-left srch-popover",
            role: "dialog",
            "aria-label": "Edit the vector input",
            onsubmit: move |e| {
                e.prevent_default();
                match apply_editor.apply(dimensions) {
                    Ok(input) => edit(r, |s| {
                        s.form.vector = input;
                        s.vector_edit = None;
                        s.vector_error = None;
                    }),
                    Err(message) => edit(r, |s| s.vector_error = Some(message)),
                }
            },
            div { class: "field",
                label { r#for: "srch-vec-kind", "Source" }
                select {
                    id: "srch-vec-kind",
                    class: "select native",
                    onchange: move |e| {
                        let v = e.value();
                        edit(r, |s| {
                            if let Some(ed) = &mut s.vector_edit {
                                ed.kind = VectorKind::ALL
                                    .into_iter()
                                    .find(|k| k.label() == v)
                                    .unwrap_or_default();
                            }
                        });
                    },
                    for k in VectorKind::ALL {
                        option { key: "{k.label()}", value: "{k.label()}", selected: k == editor.kind, "{k.label()}" }
                    }
                }
            }
            match editor.kind {
                VectorKind::Record => rsx! {
                    div { class: "field",
                        label { r#for: "srch-vec-record", "Record id" }
                        input {
                            id: "srch-vec-record",
                            class: "input mono",
                            placeholder: "e.g. art104",
                            value: "{editor.record_id}",
                            oninput: move |e| {
                                let v = e.value();
                                edit(r, |s| if let Some(ed) = &mut s.vector_edit { ed.record_id = v; });
                            },
                        }
                        span { class: "hint", "Its stored embedding is the query. The record is not excluded." }
                    }
                },
                VectorKind::Values => rsx! {
                    div { class: "field",
                        label { r#for: "srch-vec-values", "Vector ({dimensions} values)" }
                        textarea {
                            id: "srch-vec-values",
                            class: "textarea mono srch-values",
                            rows: "3",
                            placeholder: "[0.021, -0.104, 0.008, …]",
                            value: "{editor.values}",
                            oninput: move |e| {
                                let v = e.value();
                                edit(r, |s| if let Some(ed) = &mut s.vector_edit { ed.values = v; });
                            },
                        }
                    }
                },
            }
            span { class: "hint", "{hint}" }
            if let Some(message) = error {
                div { class: "field-error", role: "alert", "{message}" }
            }
            div { class: "popover-actions",
                button {
                    class: "btn",
                    r#type: "button",
                    onclick: move |_| edit(r, |s| { s.vector_edit = None; s.vector_error = None; }),
                    "Cancel"
                }
                button { class: "btn primary", r#type: "submit", "Apply" }
            }
        }
    }
}

#[component]
fn FilterPopover(draft: FilterDraft, fields: Vec<String>) -> Element {
    let ui = use_context::<SearchUi>();
    let r = ui.results;
    let ready = !draft.value.trim().is_empty() && fields.contains(&draft.field);
    let add = draft.clone();
    rsx! {
        form {
            class: "popover align-left srch-popover narrow",
            role: "dialog",
            "aria-label": "Add a filter",
            onsubmit: move |e| {
                e.prevent_default();
                let value = add.value.trim().to_string();
                if value.is_empty() { return; }
                edit(r, |s| {
                    let f = Filter { field: add.field.clone(), value };
                    if !s.form.filters.contains(&f) { s.form.filters.push(f); }
                    s.filter_edit = None;
                });
            },
            div { class: "srch-filter-row",
                select {
                    class: "select native",
                    "aria-label": "Filter field",
                    onchange: move |e| {
                        let v = e.value();
                        edit(r, |s| if let Some(d) = &mut s.filter_edit { d.field = v; });
                    },
                    for f in fields.iter() {
                        option { key: "{f}", value: "{f}", selected: *f == draft.field, "{f}" }
                    }
                }
                span { class: "srch-eq", "=" }
                input {
                    class: "input",
                    "aria-label": "Filter value",
                    placeholder: "value",
                    value: "{draft.value}",
                    oninput: move |e| {
                        let v = e.value();
                        edit(r, |s| if let Some(d) = &mut s.filter_edit { d.value = v; });
                    },
                }
            }
            div { class: "popover-actions",
                button {
                    class: "btn",
                    r#type: "button",
                    onclick: move |_| edit(r, |s| s.filter_edit = None),
                    "Cancel"
                }
                button { class: "btn primary", r#type: "submit", disabled: !ready, "Add" }
            }
        }
    }
}

#[component]
fn AdvancedRanking(fusion: FusionForm, invalid: bool) -> Element {
    let ui = use_context::<SearchUi>();
    let r = ui.results;
    let cls = if invalid {
        "input num invalid"
    } else {
        "input num"
    };
    rsx! {
        div { class: "srch-advanced-row",
            span { class: "srch-fusion", "Fusion: reciprocal rank fusion" }
            div { class: "srch-ctl",
                label { r#for: "srch-k", "Smoothing k" }
                input {
                    id: "srch-k",
                    class: "{cls}",
                    inputmode: "numeric",
                    value: "{fusion.k}",
                    title: "Added to every rank before it is inverted. Not a source weight.",
                    oninput: move |e| {
                        let v = e.value();
                        edit(r, |s| s.form.fusion.k = v);
                    },
                }
            }
            div { class: "srch-ctl",
                label { r#for: "srch-wk", "Keyword weight" }
                input {
                    id: "srch-wk",
                    class: "{cls}",
                    inputmode: "decimal",
                    value: "{fusion.keyword_weight}",
                    title: "Scales the keyword ranking's contribution. Not the smoothing constant.",
                    oninput: move |e| {
                        let v = e.value();
                        edit(r, |s| s.form.fusion.keyword_weight = v);
                    },
                }
            }
            div { class: "srch-ctl",
                label { r#for: "srch-wv", "Vector weight" }
                input {
                    id: "srch-wv",
                    class: "{cls}",
                    inputmode: "decimal",
                    value: "{fusion.vector_weight}",
                    title: "Scales the vector ranking's contribution. Not the smoothing constant.",
                    oninput: move |e| {
                        let v = e.value();
                        edit(r, |s| s.form.fusion.vector_weight = v);
                    },
                }
            }
            div { class: "srch-ctl",
                label { r#for: "srch-cand", "Candidates per source" }
                input {
                    id: "srch-cand",
                    class: "{cls}",
                    inputmode: "numeric",
                    value: "{fusion.candidates}",
                    title: "How many records each ranking holds before fusion. A deeper list can change the order.",
                    oninput: move |e| {
                        let v = e.value();
                        edit(r, |s| s.form.fusion.candidates = v);
                    },
                }
            }
        }
    }
}
