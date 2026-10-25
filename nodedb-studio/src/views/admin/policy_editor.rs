//! The Policy editor: one policy's name, target, operation, role, row
//! filter, row constraint, and state. Edits are drafts until applied from
//! the review. The bypass privilege is never assumed, and the editor says so.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::clipboard::use_copier;
use crate::components::icon::{Ico, Icon};
use crate::models::admin::AdminChange;
use crate::models::admin_policy::{PolicyChange, PolicyDraft, PolicyOperation, PolicySet};
use crate::services::backend::Backend;
use crate::state::admin::{AdminUi, PolicyUi};
use crate::views::admin::policies::Editing;
use crate::views::admin::tasks;
use crate::views::events::common::{FormField, InfoTip};

/// Lay `f` over the draft being edited and keep the result.
fn update(mut ui: Signal<PolicyUi>, base: &PolicyDraft, f: impl FnOnce(&mut PolicyDraft)) {
    let mut d = base.clone();
    f(&mut d);
    ui.write().set_draft(d);
}

#[component]
pub fn PolicyEditor(editing: Editing, set: PolicySet) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let mut ui = use_context::<AdminUi>().policies;
    let copier = use_copier();
    let d = editing.draft.clone();
    let creating = editing.stored.is_none();
    let others: Vec<_> = set.policies.iter().collect();
    let dirty = editing.dirty();
    let show = d.tried;
    let name_err = show.then(|| d.name_issue(&others)).flatten();
    let using_err = show.then(|| d.using_issue()).flatten();
    let check_err = show.then(|| d.check_issue()).flatten();
    let role_err = show.then(|| d.role_issue()).flatten();
    let has_check = d.operation.has_check();
    let valid = d.is_valid(&others);
    let review = {
        let base = d.clone();
        let stored = editing.stored.is_some();
        move |_| {
            if !valid {
                update(ui, &base, |x| x.tried = true);
                return;
            }
            let change = if stored {
                PolicyChange::Update(base.clone())
            } else {
                PolicyChange::Create(base.clone())
            };
            let (backend, change) = (backend.clone(), AdminChange::Policy(change));
            spawn_forever(async move { tasks::open_review(ui, backend, change).await });
        }
    };
    let discard = {
        let id = d.id.clone();
        move |_| {
            let mut w = ui.write();
            w.drop_draft(id.as_deref());
            if id.is_none() {
                w.selected = None;
            }
        }
    };
    rsx! {
        div { class: "ad-editor",
            div { class: "ad-form",
                FormField { label: "Policy name", id: "pol-name", error: name_err, class: "ad-inline",
                    input {
                        id: "pol-name",
                        class: "input",
                        value: "{d.name}",
                        oninput: {
                            let base = d.clone();
                            move |e| update(ui, &base, |x| x.name = e.value())
                        },
                    }
                }
                FormField { label: "Database", id: "pol-database", class: "ad-inline",
                    select { id: "pol-database", class: "select native", disabled: true,
                        option { selected: true, "{d.database}" }
                    }
                }
                FormField { label: "Collection", id: "pol-collection", class: "ad-inline",
                    if creating {
                        select {
                            id: "pol-collection",
                            class: "select native",
                            onchange: {
                                let base = d.clone();
                                move |e| update(ui, &base, |x| x.collection = e.value())
                            },
                            for c in set.collections.iter() {
                                option { key: "{c}", value: "{c}", selected: *c == d.collection, "{c}" }
                            }
                        }
                    } else {
                        input {
                            id: "pol-collection",
                            class: "input",
                            value: "{d.collection}",
                            readonly: true,
                            title: "A policy stays on its collection",
                        }
                    }
                }
                div { class: "field-row two",
                    FormField { label: "Operation", id: "pol-op",
                        select {
                            id: "pol-op",
                            class: "select native",
                            onchange: {
                                let base = d.clone();
                                move |e| {
                                    let v = e.value();
                                    if let Some(op) = PolicyOperation::ALL.iter().copied().find(|o| o.label() == v) {
                                        update(ui, &base, |x| x.operation = op);
                                    }
                                }
                            },
                            for op in PolicyOperation::ALL {
                                option { key: "{op.label()}", value: "{op.label()}", selected: op == d.operation, "{op.label()}" }
                            }
                        }
                    }
                    FormField { label: "Role", id: "pol-role", error: role_err,
                        select {
                            id: "pol-role",
                            class: "select native",
                            onchange: {
                                let base = d.clone();
                                move |e| update(ui, &base, |x| x.role = e.value())
                            },
                            if d.role.is_empty() {
                                option { value: "", selected: true, "Choose a role" }
                            }
                            for r in set.roles.iter() {
                                option { key: "{r}", value: "{r}", selected: *r == d.role, "{r}" }
                            }
                        }
                    }
                }
                FormField { label: "USING (row filter)", id: "pol-using", error: using_err,
                    div { class: "ad-expr",
                        textarea {
                            id: "pol-using",
                            class: "textarea mono",
                            rows: "3",
                            spellcheck: false,
                            value: "{d.using_expr}",
                            oninput: {
                                let base = d.clone();
                                move |e| update(ui, &base, |x| x.using_expr = e.value())
                            },
                        }
                        button {
                            class: "icon-btn sm ad-copy",
                            title: "Copy the row filter",
                            "aria-label": "Copy the row filter",
                            onclick: {
                                let text = d.using_expr.clone();
                                move |_| copier.copy(text.clone())
                            },
                            Ico { icon: Icon::Copy, size: 16 }
                        }
                    }
                }
                FormField { label: "CHECK (row constraint)", id: "pol-check", error: check_err,
                    div { class: "ad-expr",
                        textarea {
                            id: "pol-check",
                            class: "textarea mono",
                            rows: "2",
                            spellcheck: false,
                            disabled: !has_check,
                            placeholder: if has_check { "Rows a write may produce" } else { "Not applicable for {d.operation.label()}" },
                            value: if has_check { "{d.check_expr}" } else { "" },
                            oninput: {
                                let base = d.clone();
                                move |e| update(ui, &base, |x| x.check_expr = e.value())
                            },
                        }
                        button {
                            class: "icon-btn sm ad-copy",
                            title: "Copy the row constraint",
                            "aria-label": "Copy the row constraint",
                            disabled: !has_check,
                            onclick: {
                                let text = d.check_expr.clone();
                                move |_| copier.copy(text.clone())
                            },
                            Ico { icon: Icon::Copy, size: 16 }
                        }
                    }
                }
            }
            div { class: "ad-action-row",
                label { class: "cl-switch",
                    input {
                        r#type: "checkbox",
                        class: "switch",
                        checked: d.enabled,
                        onchange: {
                            let base = d.clone();
                            move |e| update(ui, &base, |x| x.enabled = e.checked())
                        },
                    }
                    "Enable policy"
                }
                div { class: "toolbar-spacer" }
                if dirty || creating {
                    button { class: "btn", onclick: discard,
                        if creating { "Discard" } else { "Discard edits" }
                    }
                }
                button {
                    class: "btn primary",
                    disabled: !dirty,
                    title: if dirty { "Read what this edit would do" } else { "Edit the policy first" },
                    onclick: review,
                    "Review changes"
                }
            }
            div { class: "kv ad-bypass",
                div { class: "kv-row",
                    span { "Bypass privilege" }
                    span { class: "ad-bypass-value",
                        "Not assumed"
                        InfoTip { text: "A role that holds a bypass privilege skips this policy. Studio never assumes a role has it.".to_string() }
                    }
                }
                div { class: "kv-row", span { "Target" } span { class: "mono", "{d.collection}" } }
            }
        }
    }
}
