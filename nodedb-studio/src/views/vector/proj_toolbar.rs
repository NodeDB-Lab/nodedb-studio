//! The projection controls, in one row: view, method, sample, seed, color
//! field, and Generate. Edits are drafts until Generate runs.

use dioxus::prelude::*;

use crate::models::vector::VectorTarget;
use crate::models::vector_projection::{
    Method, ProjectionField, ProjectionIssue, ProjectionRequest, sample_bounds,
};
use crate::state::vector::{Drafts, ProjectionUi, VectorUi};

/// Change one target's projection draft.
fn edit(mut drafts: Signal<Drafts>, key: &str, f: impl FnOnce(&mut ProjectionUi)) {
    f(drafts.write().projection_mut(key));
}

#[component]
pub fn ProjToolbar(
    target: VectorTarget,
    running: bool,
    on_generate: EventHandler<ProjectionRequest>,
    on_cancel: EventHandler<()>,
) -> Element {
    let ui = use_context::<VectorUi>();
    let drafts = ui.drafts;
    let key = target.label();
    let s = drafts.read().projection(&key);
    let checked = s.form.check(&target, &s.include);
    let issues: Vec<ProjectionIssue> = match &checked {
        Ok(_) => Vec::new(),
        Err(i) => i.clone(),
    };
    let bad = |f: ProjectionField| issues.iter().any(|i| i.field == f);
    let (min, max) = sample_bounds(&target, s.form.method);
    let ready = checked.is_ok() && !running;
    let first_issue = issues
        .first()
        .map(|i| i.message.clone())
        .unwrap_or_default();
    let color = s.form.color_by.clone().unwrap_or_default();

    rsx! {
        form {
            class: "vec-ptool",
            onsubmit: {
                let checked = checked.clone();
                move |e| {
                    e.prevent_default();
                    if ready && let Ok(req) = checked.clone() { on_generate.call(req); }
                }
            },
            div { class: "seg", role: "group", "aria-label": "Dimensions",
                button { r#type: "button", class: "active", "aria-pressed": "true", "2D" }
                button {
                    r#type: "button",
                    disabled: true,
                    "aria-pressed": "false",
                    title: "3D needs a renderer with a third axis and a camera. This build draws 2D only.",
                    "3D"
                }
            }
            div { class: "vec-ctl",
                label { r#for: "pj-method", "Method" }
                select {
                    id: "pj-method",
                    class: if bad(ProjectionField::Method) { "select native invalid" } else { "select native" },
                    onchange: {
                        let key = key.clone();
                        move |e| {
                            let v = e.value();
                            edit(drafts, &key, |st| {
                                st.form.method = Method::ALL
                                    .into_iter()
                                    .find(|m| m.label() == v)
                                    .unwrap_or(Method::Pca);
                            });
                        }
                    },
                    for m in Method::ALL {
                        option {
                            key: "{m.label()}",
                            value: "{m.label()}",
                            selected: m == s.form.method,
                            disabled: m.support().is_err(),
                            title: m.support().err().unwrap_or(""),
                            "{m.label()}"
                        }
                    }
                }
            }
            div { class: "vec-ctl",
                label { r#for: "pj-sample", "Sample" }
                input {
                    id: "pj-sample",
                    class: if bad(ProjectionField::Sample) { "input num invalid" } else { "input num" },
                    inputmode: "numeric",
                    title: "From {min} to {max} records",
                    value: "{s.form.sample}",
                    "aria-invalid": "{bad(ProjectionField::Sample)}",
                    oninput: {
                        let key = key.clone();
                        move |e| {
                            let v = e.value();
                            edit(drafts, &key, |st| st.form.sample.clone_from(&v));
                        }
                    },
                }
            }
            div { class: "vec-ctl",
                label { r#for: "pj-seed", "Seed" }
                input {
                    id: "pj-seed",
                    class: if bad(ProjectionField::Seed) { "input num seed invalid" } else { "input num seed" },
                    inputmode: "numeric",
                    value: "{s.form.seed}",
                    "aria-invalid": "{bad(ProjectionField::Seed)}",
                    oninput: {
                        let key = key.clone();
                        move |e| {
                            let v = e.value();
                            edit(drafts, &key, |st| st.form.seed.clone_from(&v));
                        }
                    },
                }
            }
            div { class: "vec-ctl",
                label { r#for: "pj-color", "Color by" }
                select {
                    id: "pj-color",
                    class: "select native",
                    onchange: {
                        let key = key.clone();
                        move |e| {
                            let v = e.value();
                            edit(drafts, &key, |st| {
                                st.form.color_by = (!v.is_empty()).then(|| v.clone());
                            });
                        }
                    },
                    option { value: "", selected: color.is_empty(), "None" }
                    for f in target.color_fields.iter() {
                        option { key: "{f}", value: "{f}", selected: *f == color, "{f}" }
                    }
                }
            }
            div { class: "toolbar-spacer" }
            if running {
                span { class: "vec-running", role: "status", "Projecting…" }
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
                    title: if ready { "Sample the index and project the sample".to_string() } else { first_issue },
                    "Generate"
                }
            }
        }
        if !issues.is_empty() {
            div { class: "error vec-issues vec-ptool-issues", role: "alert",
                for i in issues.iter() {
                    span { key: "{i.message}", "{i.message} " }
                }
            }
        }
    }
}
