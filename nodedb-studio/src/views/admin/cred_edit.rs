//! The Edit credential form: provider, secret reference, endpoint, allowed
//! models, and the dimension check, with the actions that test, rotate, and
//! review. The secret value never reaches Studio, so the masked field is a
//! placeholder, and nothing here reveals it.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::admin::AdminChange;
use crate::models::admin_credential::{
    Credential, CredentialChange, CredentialDraft, CredentialKind, CredentialList,
};
use crate::models::format::date_minute;
use crate::services::backend::Backend;
use crate::state::admin::{AdminUi, CredTestRun, CredUi};
use crate::views::admin::tasks;
use crate::views::events::common::FormField;

/// Lay `f` over the draft being edited and keep the result. A new
/// credential's draft has no alias to key it by yet, so it has its own slot.
fn update(mut ui: Signal<CredUi>, base: &CredentialDraft, f: impl FnOnce(&mut CredentialDraft)) {
    let mut d = base.clone();
    f(&mut d);
    let mut w = ui.write();
    if w.creating.is_some() {
        w.creating = Some(d);
    } else {
        w.set_draft(d);
    }
}

#[component]
pub fn CredentialEditor(credential: Option<Credential>, list: CredentialList) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let mut ui = use_context::<AdminUi>().credentials;
    let u = ui.read().clone();
    let creating = credential.is_none();
    let d = match credential.as_ref() {
        Some(c) => u.draft_of(&c.alias).cloned().unwrap_or_else(|| c.draft()),
        None => u.creating.clone().unwrap_or_else(CredentialDraft::blank),
    };
    let dirty = credential.as_ref().is_none_or(|c| d.differs_from(c));
    let taken: Vec<String> = list.items.iter().map(|c| c.alias.clone()).collect();
    let alias_err = (d.tried && creating)
        .then(|| d.alias_issue(&taken))
        .flatten();
    let embeds = d.kind == CredentialKind::Https;
    let show = d.tried;
    let secret_err = show.then(|| d.secret_issue()).flatten();
    let endpoint_err = show.then(|| d.endpoint_issue()).flatten();
    let models_err = show.then(|| d.models_issue()).flatten();
    let dim_err = d.dimension_issue();
    // The index the vectors feed. A mismatch is a warning: the server
    // refuses the write later, and the person can still edit.
    let index = list.indexes.first().cloned();
    let fit = match (d.dimension_value(), index.as_ref()) {
        (Some(dim), Some(i)) => Some((dim == i.dimension, i.clone())),
        _ => None,
    };
    let review = {
        let (backend, base, taken_ids) = (backend.clone(), d.clone(), taken.clone());
        move |_| {
            if !base.is_valid() || (creating && base.alias_issue(&taken_ids).is_some()) {
                update(ui, &base, |x| x.tried = true);
                return;
            }
            let change = if creating {
                CredentialChange::Create(base.clone())
            } else {
                CredentialChange::Update(base.clone())
            };
            let (backend, change) = (backend.clone(), AdminChange::Credential(change));
            spawn_forever(async move { tasks::open_review(ui, backend, change).await });
        }
    };
    let rotate = {
        let (backend, alias) = (backend.clone(), d.alias.clone());
        move |_| {
            let (backend, change) = (
                backend.clone(),
                AdminChange::Credential(CredentialChange::Rotate {
                    alias: alias.clone(),
                }),
            );
            spawn_forever(async move { tasks::open_review(ui, backend, change).await });
        }
    };
    let test = {
        let (backend, alias) = (backend.clone(), d.alias.clone());
        move |_| {
            let (backend, alias) = (backend.clone(), alias.clone());
            spawn_forever(async move {
                ui.write().test = CredTestRun::Running;
                let result = backend.test_credential(&alias).await;
                let mut w = ui.write();
                w.test = match result {
                    Ok(t) => CredTestRun::Done(t),
                    Err(e) => CredTestRun::Failed(e.to_string()),
                };
                // The status changed in the store. Read the list again.
                w.epoch += 1;
            });
        }
    };
    let discard = {
        let alias = d.alias.clone();
        move |_| {
            let mut w = ui.write();
            if creating {
                w.creating = None;
                w.selected = None;
            } else {
                w.drop_draft(&alias);
            }
        }
    };
    let running = matches!(u.test, CredTestRun::Running);
    rsx! {
        div { class: "ad-editor",
            div { class: "ad-form",
                FormField { label: "Alias", id: "cr-alias", class: "ad-inline", error: alias_err,
                    input {
                        id: "cr-alias",
                        class: "input",
                        value: "{d.alias}",
                        readonly: !creating,
                        spellcheck: false,
                        title: if creating { "Lowercase letters, digits, and underscores" } else { "An alias is a stable name. It cannot change." },
                        oninput: {
                            let base = d.clone();
                            move |e| update(ui, &base, |x| x.alias = e.value())
                        },
                    }
                }
                FormField { label: "Provider", id: "cr-kind", class: "ad-inline",
                    select {
                        id: "cr-kind",
                        class: "select native",
                        onchange: {
                            let base = d.clone();
                            move |e| {
                                let v = e.value();
                                if let Some(k) = CredentialKind::ALL.iter().copied().find(|k| k.label() == v) {
                                    update(ui, &base, |x| x.kind = k);
                                }
                            }
                        },
                        for k in CredentialKind::ALL {
                            option { key: "{k.label()}", value: "{k.label()}", selected: k == d.kind, "{k.label()}" }
                        }
                    }
                }
                h4 { class: "inspector-section ad-section", "Authentication" }
                FormField {
                    label: "Secret reference",
                    id: "cr-secret",
                    error: secret_err,
                    hint: "Stored securely. Never in saved queries or exported logs.".to_string(),
                    div { class: "ad-secret",
                        input {
                            id: "cr-secret",
                            class: "input mono",
                            value: "{d.secret_ref}",
                            spellcheck: false,
                            oninput: {
                                let base = d.clone();
                                move |e| update(ui, &base, |x| x.secret_ref = e.value())
                            },
                        }
                        button {
                            class: "icon-btn",
                            disabled: true,
                            title: "Studio never reads the secret value",
                            "aria-label": "Reveal the secret value",
                            Ico { icon: Icon::Eye, size: 16 }
                        }
                        input {
                            class: "input mono ad-masked",
                            r#type: "password",
                            value: "••••••••••••",
                            readonly: true,
                            "aria-label": "Secret value, hidden",
                            tabindex: "-1",
                        }
                    }
                }
                FormField { label: "Endpoint", id: "cr-endpoint", error: endpoint_err,
                    input {
                        id: "cr-endpoint",
                        class: "input",
                        value: "{d.endpoint}",
                        spellcheck: false,
                        oninput: {
                            let base = d.clone();
                            move |e| update(ui, &base, |x| x.endpoint = e.value())
                        },
                    }
                }
                if embeds {
                    FormField {
                        label: "Allowed models",
                        id: "cr-models",
                        error: models_err,
                        hint: "Comma separated, e.g. support-embed-v2, embed-large.".to_string(),
                        input {
                            id: "cr-models",
                            class: "input",
                            value: "{d.models}",
                            spellcheck: false,
                            oninput: {
                                let base = d.clone();
                                move |e| update(ui, &base, |x| x.models = e.value())
                            },
                        }
                    }
                    h4 { class: "inspector-section ad-section", "Compatibility" }
                    FormField {
                        label: "Dimension check",
                        id: "cr-dim",
                        class: "ad-inline",
                        error: dim_err,
                        hint: "Must match the index dimension.".to_string(),
                        input {
                            id: "cr-dim",
                            class: "input ad-dim",
                            r#type: "number",
                            min: "1",
                            value: "{d.dimension}",
                            oninput: {
                                let base = d.clone();
                                move |e| update(ui, &base, |x| x.dimension = e.value())
                            },
                        }
                    }
                    if let Some((ok, i)) = fit {
                        div { class: if ok { "callout ok" } else { "callout warn" },
                            Ico { icon: if ok { Icon::CircleCheck } else { Icon::TriangleAlert }, size: 16 }
                            span {
                                if ok {
                                    "Matches {i.index} ({i.dimension} dimensions)."
                                } else {
                                    "{i.index} on {i.collection} expects {i.dimension} dimensions. Vectors of another size do not fit."
                                }
                            }
                        }
                    }
                }
            }
            match u.test.clone() {
                CredTestRun::Idle => rsx! {},
                CredTestRun::Running => rsx! { div { class: "async-loading", "Testing…" } },
                CredTestRun::Failed(msg) => rsx! {
                    div { class: "callout err", role: "alert",
                        Ico { icon: Icon::CircleAlert, size: 16 }
                        span { "{msg}" }
                    }
                },
                CredTestRun::Done(t) => rsx! {
                    div { class: if t.reachable { "callout ok" } else { "callout err" }, role: "status",
                        Ico { icon: if t.reachable { Icon::CircleCheck } else { Icon::CircleAlert }, size: 16 }
                        span {
                            "{t.message}"
                            if let Some(ms) = t.latency_ms {
                                " Answered in {ms} ms."
                            }
                            if let (Some(n), Some(with)) = (t.dimension, t.compared_with.as_ref()) {
                                " Returned {n} dimensions, compared with {with}."
                            }
                            match t.compatible(index.as_ref().map(|i| i.dimension)) {
                                Some(true) => " The dimension fits the index.",
                                Some(false) => " The dimension does not fit the index.",
                                None => "",
                            }
                        }
                    }
                },
            }
            div { class: "ad-buttons ad-cred-actions",
                button {
                    class: "btn primary",
                    disabled: running || dirty || creating,
                    title: if dirty || creating { "Create the credential, and apply or discard edits first. A test runs the saved credential." } else { "Send one request to the provider. Nothing calls it on its own." },
                    onclick: test,
                    Ico { icon: Icon::Link, size: 16 }
                    "Test connection"
                }
                button {
                    class: "btn",
                    disabled: dirty || creating,
                    title: if dirty || creating { "Create the credential first, and apply or discard edits" } else { "Ask the secret store for a new version" },
                    onclick: rotate,
                    Ico { icon: Icon::RefreshCw, size: 16 }
                    "Rotate credential"
                }
                button {
                    class: "btn",
                    disabled: !dirty,
                    title: if dirty { "Read what these edits would do" } else { "Edit the credential first" },
                    onclick: review,
                    Ico { icon: Icon::FileText, size: 16 }
                    "Review changes"
                }
                if dirty {
                    button { class: "btn ghost", onclick: discard, "Discard edits" }
                }
            }
            div { class: "meta-line ad-cred-note",
                Ico { icon: Icon::Info, size: 16 }
                match credential.as_ref() {
                    None => rsx! { span { "A new credential reads Unknown until you test it. Nothing calls it on its own." } },
                    Some(c) => rsx! {
                        if c.enabled {
                            span { "Provider enabled for explicit requests. Nothing calls it on its own." }
                        } else {
                            span { "Provider not enabled. Nothing calls it." }
                        }
                        match c.last_tested {
                            Some(t) => rsx! { span { "Last tested {date_minute(t, 0)} UTC." } },
                            None => rsx! { span { "Never tested." } },
                        }
                        span { "Secret version {c.secret_version}." }
                    },
                }
            }
        }
    }
}
