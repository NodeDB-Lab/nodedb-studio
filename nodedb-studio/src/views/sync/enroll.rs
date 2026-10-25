//! The Enroll Lite device sheet. The Origin issues a one-use token and the
//! device presents it. The token is a secret: this sheet shows it masked and
//! copies the command that carries it. Nothing here renders the raw value.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::components::sheet::{Sheet, SheetFooter, SheetWidth};
use crate::models::format::clock;
use crate::models::sync_enroll::EnrollDraft;
use crate::services::backend::Backend;
use crate::state::sync::SyncUi;
use crate::views::events::common::FormField;
use crate::views::sync::enroll_tasks;

fn edit(mut ui: Signal<crate::state::sync::DevicesUi>, change: impl FnOnce(&mut EnrollDraft)) {
    if let Some(d) = ui.write().enroll.as_mut() {
        change(d);
        d.error = None;
    }
}

#[component]
pub fn EnrollSheet(draft: EnrollDraft) -> Element {
    let backend = use_context::<Rc<dyn Backend>>();
    let conn = use_context::<Memo<Option<String>>>();
    let mut ui = use_context::<SyncUi>().devices;
    let options = use_resource({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            let _conn = conn();
            async move { backend.enroll_options().await }
        }
    });
    let Some(Ok(o)) = options.read().clone() else {
        return rsx! {};
    };
    let issued = draft.issued.clone();
    let name_error = draft.tried.then(|| draft.name_issue()).flatten();
    let preset = o.presets.iter().find(|p| p.name == draft.preset).cloned();
    let ttl_min = o.token_ttl / 60;
    let token_text = match issued.as_ref() {
        Some(i) => format!(
            "{} · one use · expires in {ttl_min} min ({} UTC)",
            i.masked,
            clock(i.expires_at, 0)
        ),
        None => "Not created".to_string(),
    };
    let device_id = issued
        .as_ref()
        .map(|i| i.device_id.clone())
        .unwrap_or_else(|| o.device_id.clone());
    let locked = issued.is_some();
    let mut close = move || ui.write().enroll = None;
    let create = {
        let backend = backend.clone();
        move |_| {
            let backend = backend.clone();
            spawn_forever(async move { enroll_tasks::create(ui, backend).await });
        }
    };
    let copy = {
        let backend = backend.clone();
        move |_| {
            let backend = backend.clone();
            spawn_forever(async move { enroll_tasks::copy_command(ui, backend).await });
        }
    };
    rsx! {
        Sheet {
            title: "Enroll Lite device".to_string(),
            width: SheetWidth::Medium,
            on_close: move |_| close(),
            footer: rsx! {
                SheetFooter {
                    error: draft.error.clone(),
                    hint: Some(
                        if locked {
                            "The device registers when it redeems the token. Until then it is not listed."
                                .to_string()
                        } else {
                            "Creates a one-use token. It grants only the role and database chosen here."
                                .to_string()
                        },
                    ),
                    if locked {
                        button { class: "btn primary", onclick: move |_| close(), "Done" }
                    } else {
                        button { class: "btn", disabled: draft.busy, onclick: move |_| close(), "Cancel" }
                        button {
                            class: "btn primary",
                            disabled: draft.busy,
                            onclick: create,
                            "Create token"
                        }
                    }
                }
            },
            div { class: "sheet-form sy-form",
                FormField {
                    label: "Device name",
                    id: "enr-name",
                    hint: Some("A friendly name to identify this device.".to_string()),
                    error: name_error.clone(),
                    input {
                        id: "enr-name",
                        class: if name_error.is_some() { "input invalid" } else { "input" },
                        value: "{draft.name}",
                        readonly: locked,
                        placeholder: "Checkout iPad",
                        oninput: move |e| {
                            let v = e.value();
                            edit(ui, |d| d.name = v);
                        },
                    }
                }
                FormField {
                    label: "Origin",
                    id: "enr-origin",
                    hint: Some(
                        "Endpoint secured. The device syncs with this cluster, across every database it may read."
                            .to_string(),
                    ),
                    input {
                        id: "enr-origin",
                        class: "input",
                        readonly: true,
                        value: "{o.origin}",
                        title: "{o.endpoint}",
                    }
                }
                FormField {
                    label: "Device identity",
                    id: "enr-id",
                    hint: Some("Generated automatically.".to_string()),
                    input { id: "enr-id", class: "input mono", readonly: true, value: "{device_id}" }
                }
                FormField {
                    label: "Enrollment token",
                    id: "enr-token",
                    hint: Some("A secure token for device enrollment. It is shown masked.".to_string()),
                    input {
                        id: "enr-token",
                        class: if locked { "input mono" } else { "input" },
                        readonly: true,
                        value: "{token_text}",
                    }
                }
                FormField {
                    label: "Enrollment command",
                    id: "enr-command",
                    hint: Some(
                        "Run this command on the device. The token will not be shown again.".to_string(),
                    ),
                    div { class: "sy-command",
                        if locked {
                            button {
                                id: "enr-command",
                                class: "btn",
                                disabled: draft.copied,
                                onclick: copy,
                                Ico { icon: if draft.copied { Icon::Check } else { Icon::Copy }, size: 16 }
                                if draft.copied {
                                    "Command copied"
                                } else {
                                    "Copy enrollment command"
                                }
                            }
                        } else {
                            input {
                                id: "enr-command",
                                class: "input",
                                readonly: true,
                                value: "Available after token creation",
                            }
                        }
                    }
                }
                FormField { label: "Allowed databases", id: "enr-db",
                    select {
                        id: "enr-db",
                        class: "select native",
                        disabled: locked,
                        onchange: move |e| {
                            let v = e.value();
                            edit(ui, |d| d.database = v);
                        },
                        for db in o.databases.iter() {
                            option { key: "{db}", value: "{db}", selected: *db == draft.database, "{db}" }
                        }
                    }
                }
                FormField {
                    label: "Subscription preset",
                    id: "enr-preset",
                    hint: preset
                        .as_ref()
                        .map(|p| format!("Creates {}.", p.shapes.join(" and "))),
                    select {
                        id: "enr-preset",
                        class: "select native",
                        disabled: locked,
                        onchange: {
                            let presets = o.presets.clone();
                            move |e| {
                                let v = e.value();
                                let first = presets
                                    .iter()
                                    .find(|p| p.name == v)
                                    .and_then(|p| p.values.first().cloned())
                                    .unwrap_or_default();
                                edit(ui, |d| {
                                    d.preset = v;
                                    d.value = first;
                                });
                            }
                        },
                        for p in o.presets.iter() {
                            option { key: "{p.name}", value: "{p.name}", selected: p.name == draft.preset, "{p.name}" }
                        }
                    }
                }
                if let Some(p) = preset.as_ref() {
                    FormField { label: p.parameter_label.clone(), id: "enr-value",
                        select {
                            id: "enr-value",
                            class: "select native",
                            disabled: locked,
                            onchange: move |e| {
                                let v = e.value();
                                edit(ui, |d| d.value = v);
                            },
                            for v in p.values.iter() {
                                option { key: "{v}", value: "{v}", selected: *v == draft.value, "{v}" }
                            }
                        }
                    }
                }
                FormField {
                    label: "Least privilege role",
                    id: "enr-role",
                    hint: Some("The device authenticates as this role and can do nothing else.".to_string()),
                    select {
                        id: "enr-role",
                        class: "select native",
                        disabled: locked,
                        onchange: move |e| {
                            let v = e.value();
                            edit(ui, |d| d.role = v);
                        },
                        for r in o.roles.iter() {
                            option { key: "{r}", value: "{r}", selected: *r == draft.role, "{r}" }
                        }
                    }
                }
            }
        }
    }
}
