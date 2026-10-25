//! The Query panel: where the query vector comes from, how many neighbors,
//! and the candidate filters. Nothing runs until Find similar.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::records::Filter;
use crate::models::vector::{
    Issue, IssueField, MAX_EF_SEARCH, MAX_TOP_K, NeighborRequest, SourceKind, VectorTarget,
};
use crate::state::vector::{Drafts, SearchUi, VectorUi};

/// Change one target's search draft.
fn edit(mut drafts: Signal<Drafts>, key: &str, f: impl FnOnce(&mut SearchUi)) {
    f(drafts.write().search_mut(key));
}

#[component]
pub fn QueryBody(
    target: VectorTarget,
    provider: Option<String>,
    running: bool,
    on_run: EventHandler<NeighborRequest>,
    on_cancel: EventHandler<()>,
) -> Element {
    let ui = use_context::<VectorUi>();
    let drafts = ui.drafts;
    let key = target.label();
    let s = drafts.read().search(&key);
    let form = s.form.clone();
    let checked = form.check(&target);
    let issues: Vec<Issue> = match &checked {
        Ok(_) => Vec::new(),
        Err(i) => i.clone(),
    };
    // An empty source is "not started", not an error to show.
    let shown: Vec<&Issue> = issues
        .iter()
        .filter(|i| match i.field {
            IssueField::RecordId => !form.record_id.trim().is_empty(),
            IssueField::Vector => !form.vector_text.trim().is_empty(),
            IssueField::TopK | IssueField::EfSearch | IssueField::Filter => true,
        })
        .collect();
    let has = |f: IssueField| shown.iter().any(|i| i.field == f);
    let first_issue = issues.first().map(|i| i.message.clone());
    let ready = checked.is_ok() && !running;
    let text_hint = match &provider {
        Some(name) => format!(
            "Text queries need {name} to embed them. This tab starts from a record or a vector."
        ),
        None => "Text queries need an embedding provider. None is configured.".to_string(),
    };
    let source_label = form.source.label();
    let no_filter_fields = target.filter_fields.is_empty();

    rsx! {
        form {
            class: "vec-form",
            onsubmit: {
                let checked = checked.clone();
                move |e| {
                    e.prevent_default();
                    if ready && let Ok(req) = checked.clone() { on_run.call(req); }
                }
            },
            div { class: "vec-row",
                div { class: "field vec-source",
                    label { r#for: "vec-source", "Source" }
                    select {
                        id: "vec-source",
                        class: "select native",
                        onchange: {
                            let key = key.clone();
                            move |e| {
                                let v = e.value();
                                edit(drafts, &key, |st| {
                                    st.form.source = SourceKind::ALL
                                        .into_iter()
                                        .find(|k| k.label() == v)
                                        .unwrap_or(SourceKind::Record);
                                });
                            }
                        },
                        for k in SourceKind::ALL {
                            option { key: "{k.label()}", value: "{k.label()}", selected: k == form.source, "{k.label()}" }
                        }
                        option { value: "Text", disabled: true, title: "{text_hint}", "Text" }
                    }
                }
                match form.source {
                    SourceKind::Record => rsx! {
                        div { class: "field grow",
                            label { r#for: "vec-record", "Record ID" }
                            input {
                                id: "vec-record",
                                class: if has(IssueField::RecordId) { "input mono invalid" } else { "input mono" },
                                placeholder: "{target.collection}/…, e.g. art104",
                                value: "{form.record_id}",
                                "aria-invalid": "{has(IssueField::RecordId)}",
                                oninput: {
                                    let key = key.clone();
                                    move |e| {
                                        let v = e.value();
                                        edit(drafts, &key, |st| st.form.record_id.clone_from(&v));
                                    }
                                },
                            }
                        }
                    },
                    SourceKind::Vector => rsx! {
                        div { class: "field grow",
                            label { r#for: "vec-vector", "Query vector ({target.dimensions} values)" }
                            textarea {
                                id: "vec-vector",
                                class: if has(IssueField::Vector) { "textarea mono invalid vec-vector" } else { "textarea mono vec-vector" },
                                rows: "2",
                                placeholder: "[0.021, -0.104, 0.008, …]",
                                value: "{form.vector_text}",
                                "aria-invalid": "{has(IssueField::Vector)}",
                                oninput: {
                                    let key = key.clone();
                                    move |e| {
                                        let v = e.value();
                                        edit(drafts, &key, |st| st.form.vector_text.clone_from(&v));
                                    }
                                },
                            }
                        }
                    },
                }
                div { class: "field vec-topk",
                    label { r#for: "vec-topk", "Top K" }
                    input {
                        id: "vec-topk",
                        class: if has(IssueField::TopK) { "input invalid" } else { "input" },
                        r#type: "number",
                        min: "1",
                        max: "{MAX_TOP_K}",
                        value: "{form.top_k}",
                        "aria-invalid": "{has(IssueField::TopK)}",
                        oninput: {
                            let key = key.clone();
                            move |e| {
                                let v = e.value();
                                edit(drafts, &key, |st| st.form.top_k.clone_from(&v));
                            }
                        },
                    }
                }
            }
            div { class: "vec-row actions",
                label {
                    class: "check-row",
                    title: if form.source == SourceKind::Vector { "A pasted vector has no record to exclude" } else { "" },
                    input {
                        r#type: "checkbox",
                        class: "checkbox",
                        checked: form.exclude_source && form.source == SourceKind::Record,
                        disabled: form.source == SourceKind::Vector,
                        onchange: {
                            let key = key.clone();
                            move |e| {
                                let on = e.checked();
                                edit(drafts, &key, |st| st.form.exclude_source = on);
                            }
                        },
                    }
                    "Exclude query record"
                }
                button {
                    class: "btn",
                    r#type: "button",
                    disabled: no_filter_fields,
                    title: if no_filter_fields { "This index has no filterable field" } else { "Keep only candidates that match, before the top-k cut" },
                    onclick: {
                        let key = key.clone();
                        let field = target.filter_fields.first().cloned().unwrap_or_default();
                        move |_| {
                            let field = field.clone();
                            edit(drafts, &key, |st| st.form.filters.push(Filter { field: field.clone(), value: String::new() }));
                        }
                    },
                    Ico { icon: Icon::Plus, size: 16 }
                    "Filter"
                }
                button {
                    class: "btn ghost",
                    r#type: "button",
                    "aria-expanded": "{s.advanced}",
                    onclick: {
                        let key = key.clone();
                        move |_| edit(drafts, &key, |st| st.advanced = !st.advanced)
                    },
                    Ico { icon: if s.advanced { Icon::ChevronDown } else { Icon::ChevronRight }, size: 16 }
                    "Advanced"
                }
                div { class: "toolbar-spacer" }
                if running {
                    span { class: "vec-running", role: "status", "Searching…" }
                    button {
                        class: "btn",
                        r#type: "button",
                        onclick: move |_| on_cancel.call(()),
                        "Cancel"
                    }
                } else {
                    button {
                        class: "btn primary",
                        r#type: "submit",
                        disabled: !ready,
                        title: if ready { "Rank the nearest neighbors".to_string() } else { first_issue.clone().unwrap_or_default() },
                        "Find similar"
                    }
                }
            }
            if !form.filters.is_empty() {
                div { class: "vec-filters",
                    for (i , f) in form.filters.iter().enumerate() {
                        {
                            let key_f = key.clone();
                            let key_v = key.clone();
                            let key_x = key.clone();
                            rsx! {
                                div { key: "{i}-{f.field}", class: "vec-filter",
                                    label { class: "sr-only", r#for: "vec-ff-{i}", "Filter field" }
                                    select {
                                        id: "vec-ff-{i}",
                                        class: "select native",
                                        onchange: move |e| {
                                            let v = e.value();
                                            edit(drafts, &key_f, |st| { if let Some(x) = st.form.filters.get_mut(i) { x.field.clone_from(&v); } });
                                        },
                                        for name in target.filter_fields.iter() {
                                            option { key: "{name}", value: "{name}", selected: *name == f.field, "{name}" }
                                        }
                                    }
                                    span { class: "vec-eq", "=" }
                                    label { class: "sr-only", r#for: "vec-fv-{i}", "Filter value" }
                                    input {
                                        id: "vec-fv-{i}",
                                        class: "input",
                                        placeholder: "value",
                                        value: "{f.value}",
                                        oninput: move |e| {
                                            let v = e.value();
                                            edit(drafts, &key_v, |st| { if let Some(x) = st.form.filters.get_mut(i) { x.value.clone_from(&v); } });
                                        },
                                    }
                                    button {
                                        class: "icon-btn sm",
                                        r#type: "button",
                                        title: "Remove filter",
                                        "aria-label": "Remove filter on {f.field}",
                                        onclick: move |_| edit(drafts, &key_x, |st| { if i < st.form.filters.len() { st.form.filters.remove(i); } }),
                                        Ico { icon: Icon::X, size: 14 }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if s.advanced {
                div { class: "vec-advanced",
                    div { class: "field vec-ef",
                        label { r#for: "vec-ef", "Beam width (ef_search)" }
                        input {
                            id: "vec-ef",
                            class: if has(IssueField::EfSearch) { "input invalid" } else { "input" },
                            r#type: "number",
                            min: "1",
                            max: "{MAX_EF_SEARCH}",
                            placeholder: "64",
                            value: "{form.ef_search}",
                            oninput: {
                                let key = key.clone();
                                move |e| {
                                    let v = e.value();
                                    edit(drafts, &key, |st| st.form.ef_search.clone_from(&v));
                                }
                            },
                        }
                    }
                    p { class: "hint",
                        "HNSW beam width. Empty uses the server default. A wider beam reads more candidates per query."
                    }
                }
            }
            if !shown.is_empty() {
                div { class: "error vec-issues", role: "alert",
                    for i in shown.iter() {
                        span { key: "{i.message}", "{i.message} " }
                    }
                }
            }
            if form.source == SourceKind::Vector {
                p { class: "hint vec-note",
                    "Model compatibility of a pasted vector is unconfirmed. Only its length is checked against {target.label()}."
                }
            }
            span { class: "sr-only", "Source {source_label}" }
        }
    }
}
