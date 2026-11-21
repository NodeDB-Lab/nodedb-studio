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

/// What the editor shows for a field NodeDB 0.6.2 does not report.
const FILTER_NOT_REPORTED: &str = "Filter text not reported by NodeDB 0.6.2";
const ROLE_NOT_REPORTED: &str = "Role not reported by NodeDB 0.6.2";

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
    // A stored policy whose filter or role the server does not report: the
    // editor shows that, read-only, and never an empty filter.
    let unreported = editing.stored.as_ref().is_some_and(|p| !p.is_reported());
    let others: Vec<_> = set.policies.iter().collect();
    let dirty = editing.dirty();
    let show = d.tried;
    let name_err = show.then(|| d.name_issue(&others)).flatten();
    // Nobody can fill a field the server does not report, so it raises no issue.
    let using_err = (show && !unreported).then(|| d.using_issue()).flatten();
    let check_err = show.then(|| d.check_issue()).flatten();
    let role_err = (show && !unreported).then(|| d.role_issue()).flatten();
    let has_check = d.operation.has_check();
    let valid = if unreported {
        d.name_issue(&others).is_none() && d.check_issue().is_none()
    } else {
        d.is_valid(&others)
    };
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
                        if unreported {
                            input {
                                id: "pol-role",
                                class: "input muted",
                                value: "{ROLE_NOT_REPORTED}",
                                readonly: true,
                            }
                        } else {
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
                }
                FormField { label: "USING (row filter)", id: "pol-using", error: using_err,
                    div { class: "ad-expr",
                        textarea {
                            id: "pol-using",
                            class: "textarea mono",
                            rows: "3",
                            spellcheck: false,
                            readonly: unreported,
                            value: if unreported { FILTER_NOT_REPORTED.to_string() } else { d.using_expr.clone() },
                            oninput: {
                                let base = d.clone();
                                move |e| update(ui, &base, |x| x.using_expr = e.value())
                            },
                        }
                        button {
                            class: "icon-btn sm ad-copy",
                            title: "Copy the row filter",
                            "aria-label": "Copy the row filter",
                            disabled: unreported,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::admin_policy::Policy;
    use crate::services::nodedb::NodeDbConnectionService;

    fn render(policy: Policy) -> String {
        #[derive(Clone, PartialEq)]
        struct Seed(Policy);
        fn app(seed: Seed) -> Element {
            AdminUi::provide();
            use_context_provider(|| Rc::new(NodeDbConnectionService::default()) as Rc<dyn Backend>);
            let p = seed.0;
            let set = PolicySet {
                database: p.database.clone(),
                databases: vec![p.database.clone()],
                collections: vec![p.collection.clone()],
                roles: vec!["readonly".to_string()],
                test_users: vec![],
                policies: vec![p.clone()],
                observed_at: 0,
            };
            let editing = Editing {
                draft: p.draft(),
                stored: Some(p),
            };
            rsx! { PolicyEditor { editing, set } }
        }
        let mut dom = VirtualDom::new_with_props(app, Seed(policy));
        dom.rebuild_in_place();
        dioxus_ssr::render(&dom)
    }

    fn policy(using_expr: Option<&str>, role: Option<&str>) -> Policy {
        Policy {
            id: "c.p".into(),
            name: "p".into(),
            database: "default".into(),
            collection: "c".into(),
            operation: PolicyOperation::Select,
            role: role.map(str::to_string),
            using_expr: using_expr.map(str::to_string),
            check_expr: None,
            enabled: true,
        }
    }

    #[test]
    fn a_filter_the_server_does_not_report_is_said_and_read_only() {
        let html = render(policy(None, None));
        assert!(html.contains(FILTER_NOT_REPORTED), "{html}");
        assert!(html.contains(ROLE_NOT_REPORTED), "{html}");
        assert!(!html.contains("Choose a role"), "{html}");
        let at = html.find("id=\"pol-using\"").expect("filter box");
        let tag = &html[html[..at].rfind("<textarea").expect("tag")..];
        let tag = &tag[..tag.find('>').expect("end")];
        assert!(tag.contains("readonly"), "{tag}");
    }

    #[test]
    fn a_reported_filter_is_shown_and_editable() {
        let html = render(policy(Some("id = $auth.id"), Some("readonly")));
        assert!(html.contains("id = $auth.id"), "{html}");
        assert!(!html.contains(FILTER_NOT_REPORTED));
    }
}
