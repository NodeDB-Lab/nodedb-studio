//! The connection dialog's field groups. Each group reads the shared
//! `ConnectionForm` signal and writes through `edit`, which also drops a
//! stale Test result: a pass describes the fields as they were tested.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::connections::{AuthMethod, ConnectionKind};
use crate::state::connection_form::{ConnectionForm, Field, FieldError, TestState};

/// Apply a change to the form and drop a Test result that no longer matches.
pub(super) fn edit(form: &mut Signal<ConnectionForm>, apply: impl FnOnce(&mut ConnectionForm)) {
    let mut f = form.write();
    apply(&mut f);
    if f.test != TestState::Running {
        f.test = TestState::Idle;
    }
}

fn message(errors: &[FieldError], field: Field) -> Option<&'static str> {
    errors.iter().find(|e| e.field == field).map(|e| e.message)
}

/// One label and its control. The error shows under the control.
#[component]
fn FormRow(label: &'static str, error: Option<&'static str>, children: Element) -> Element {
    rsx! {
        div { class: "fgrid-row",
            span { class: "fgrid-label", "{label}" }
            div { class: "fgrid-control",
                {children}
                if let Some(e) = error {
                    div { class: "field-error", role: "alert", "{e}" }
                }
            }
        }
    }
}

#[component]
pub fn NameAndTarget(errors: Vec<FieldError>) -> Element {
    let mut form = use_context::<Signal<ConnectionForm>>();
    let f = form.read().clone();
    rsx! {
        FormRow { label: "Name", error: message(&errors, Field::Name),
            input {
                class: if message(&errors, Field::Name).is_some() { "input invalid" } else { "input" },
                value: "{f.name}",
                autofocus: f.editing.is_none(),
                "aria-label": "Name",
                oninput: move |e| { let v = e.value(); edit(&mut form, |f| f.name = v) },
            }
        }
        FormRow { label: "Target", error: None,
            div { class: "choice-group", role: "radiogroup", "aria-label": "Target",
                for kind in [ConnectionKind::Origin, ConnectionKind::Lite] {
                    label {
                        key: "{kind.label()}",
                        class: if f.kind == kind { "choice selected" } else { "choice" },
                        input {
                            r#type: "radio",
                            class: "radio",
                            name: "target",
                            checked: f.kind == kind,
                            onchange: move |_| edit(&mut form, |f| f.kind = kind),
                        }
                        span {
                            match kind {
                                ConnectionKind::Origin => "Origin cluster",
                                ConnectionKind::Lite => "Local Lite",
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn LiteFields(errors: Vec<FieldError>) -> Element {
    let mut form = use_context::<Signal<ConnectionForm>>();
    let f = form.read().clone();
    rsx! {
        h3 { class: "form-section", "Connection details" }
        FormRow { label: "Database file", error: message(&errors, Field::LitePath),
            div { class: "input-with-button",
                input {
                    class: if message(&errors, Field::LitePath).is_some() { "input mono invalid" } else { "input mono" },
                    value: "{f.lite_path}",
                    placeholder: "/path/to/data.nodedb",
                    "aria-label": "Database file",
                    oninput: move |e| { let v = e.value(); edit(&mut form, |f| f.lite_path = v) },
                }
                BrowseButton {}
            }
        }
    }
}

/// The file picker needs a native dialog, which Studio does not ship yet.
#[component]
fn BrowseButton() -> Element {
    rsx! {
        button {
            class: "btn",
            r#type: "button",
            disabled: true,
            title: "The file picker is not available yet: type the path",
            "Browse"
        }
    }
}

#[component]
pub fn OriginFields(errors: Vec<FieldError>) -> Element {
    let mut form = use_context::<Signal<ConnectionForm>>();
    let f = form.read().clone();
    let host_error = message(&errors, Field::Host);
    let port_error = message(&errors, Field::Port);
    rsx! {
        h3 { class: "form-section", "Connection details" }
        FormRow { label: "Host", error: host_error.or(port_error),
            div { class: "host-port",
                input {
                    class: if host_error.is_some() { "input invalid" } else { "input" },
                    value: "{f.host}",
                    placeholder: "db.example.com",
                    "aria-label": "Host",
                    oninput: move |e| { let v = e.value(); edit(&mut form, |f| f.host = v) },
                }
                span { class: "fgrid-label inline", "Port" }
                input {
                    class: if port_error.is_some() { "input num invalid" } else { "input num" },
                    value: "{f.port}",
                    inputmode: "numeric",
                    "aria-label": "Port",
                    oninput: move |e| { let v = e.value(); edit(&mut form, |f| f.port = v) },
                }
            }
        }
        FormRow { label: "Database", error: None,
            input {
                class: "input",
                value: "{f.database}",
                placeholder: "Server default",
                "aria-label": "Database",
                oninput: move |e| { let v = e.value(); edit(&mut form, |f| f.database = v) },
            }
        }
    }
}

#[component]
pub fn AuthFields(errors: Vec<FieldError>) -> Element {
    let mut form = use_context::<Signal<ConnectionForm>>();
    let f = form.read().clone();
    let editing = f.editing.is_some();
    let secret_placeholder = if editing { "Unchanged" } else { "" };
    let secret_label = if f.auth == AuthMethod::Token {
        "Token"
    } else {
        "Password"
    };
    rsx! {
        div { class: "fgrid-row auth-row",
            h3 { class: "form-section inline", "Authentication" }
            div { class: "radio-inline", role: "radiogroup", "aria-label": "Authentication",
                for method in AuthMethod::ALL {
                    label { key: "{method.label()}", class: "check-row",
                        input {
                            r#type: "radio",
                            class: "radio",
                            name: "auth",
                            checked: f.auth == method,
                            onchange: move |_| edit(&mut form, |f| f.auth = method),
                        }
                        span { class: if f.auth == method { "radio-label selected" } else { "radio-label" },
                            "{method.label()}"
                        }
                    }
                }
            }
        }
        match f.auth {
            AuthMethod::UserPassword => rsx! {
                FormRow { label: "Username", error: message(&errors, Field::Username),
                    input {
                        class: if message(&errors, Field::Username).is_some() { "input invalid" } else { "input" },
                        value: "{f.username}",
                        autocomplete: "off",
                        "aria-label": "Username",
                        oninput: move |e| { let v = e.value(); edit(&mut form, |f| f.username = v) },
                    }
                }
                FormRow { label: "Password", error: None,
                    div { class: "secret-row",
                        input {
                            class: "input",
                            r#type: "password",
                            value: "{f.secret}",
                            placeholder: "{secret_placeholder}",
                            autocomplete: "off",
                            "aria-label": "{secret_label}",
                            oninput: move |e| { let v = e.value(); edit(&mut form, |f| f.secret = v) },
                        }
                        KeychainCheck {}
                    }
                }
            },
            AuthMethod::Token => rsx! {
                FormRow { label: "Token", error: message(&errors, Field::Secret),
                    div { class: "secret-row",
                        input {
                            class: if message(&errors, Field::Secret).is_some() { "input invalid" } else { "input" },
                            r#type: "password",
                            value: "{f.secret}",
                            placeholder: "{secret_placeholder}",
                            autocomplete: "off",
                            "aria-label": "{secret_label}",
                            oninput: move |e| { let v = e.value(); edit(&mut form, |f| f.secret = v) },
                        }
                        KeychainCheck {}
                    }
                }
            },
            AuthMethod::SystemCredentials => rsx! {
                p { class: "form-note", "The operating system supplies the identity. Studio stores no secret." }
            },
        }
    }
}

#[component]
fn KeychainCheck() -> Element {
    let mut form = use_context::<Signal<ConnectionForm>>();
    let keychain = form.read().keychain;
    rsx! {
        label { class: "check-row",
            input {
                r#type: "checkbox",
                class: "checkbox",
                checked: keychain,
                onchange: move |e| { let v = e.checked(); edit(&mut form, |f| f.keychain = v) },
            }
            "Save in keychain"
        }
    }
}

#[component]
pub fn TlsFields() -> Element {
    let mut form = use_context::<Signal<ConnectionForm>>();
    let f = form.read().clone();
    let name_placeholder = f.host.trim().to_string();
    rsx! {
        div { class: "fgrid-row auth-row",
            h3 { class: "form-section inline", "TLS" }
            label { class: "check-row",
                input {
                    r#type: "checkbox",
                    class: "checkbox",
                    checked: f.tls_required,
                    onchange: move |e| { let v = e.checked(); edit(&mut form, |f| f.tls_required = v) },
                }
                "Require TLS (recommended)"
            }
        }
        if f.tls_required {
            FormRow { label: "Server name", error: None,
                input {
                    class: "input",
                    value: "{f.server_name}",
                    placeholder: "{name_placeholder}",
                    "aria-label": "Server name",
                    oninput: move |e| { let v = e.value(); edit(&mut form, |f| f.server_name = v) },
                }
            }
            FormRow { label: "CA file", error: None,
                div { class: "input-with-button",
                    input {
                        class: "input mono",
                        value: "{f.ca_file}",
                        placeholder: "System roots",
                        "aria-label": "CA file",
                        oninput: move |e| { let v = e.value(); edit(&mut form, |f| f.ca_file = v) },
                    }
                    BrowseButton {}
                }
            }
            button {
                class: "disclosure",
                r#type: "button",
                "aria-expanded": "{f.client_cert_open}",
                onclick: move |_| {
                    let open = !form.peek().client_cert_open;
                    form.write().client_cert_open = open;
                },
                Ico {
                    icon: if f.client_cert_open { Icon::ChevronDown } else { Icon::ChevronRight },
                    size: 16,
                }
                "Client certificate (optional)"
            }
            if f.client_cert_open {
                FormRow { label: "Certificate", error: None,
                    div { class: "input-with-button",
                        input {
                            class: "input mono",
                            value: "{f.client_cert}",
                            placeholder: "client.pem",
                            "aria-label": "Client certificate file",
                            oninput: move |e| { let v = e.value(); edit(&mut form, |f| f.client_cert = v) },
                        }
                        BrowseButton {}
                    }
                }
                FormRow { label: "Key", error: None,
                    div { class: "input-with-button",
                        input {
                            class: "input mono",
                            value: "{f.client_key}",
                            placeholder: "client.key",
                            "aria-label": "Client key file",
                            oninput: move |e| { let v = e.value(); edit(&mut form, |f| f.client_key = v) },
                        }
                        BrowseButton {}
                    }
                }
            }
        }
    }
}
