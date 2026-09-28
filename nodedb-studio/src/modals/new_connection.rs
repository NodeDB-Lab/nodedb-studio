//! New-connection modal body.
//!
//! Single-engine client: there is no Engine picker. The Protocol field is
//! omitted too, because NodeDB's transport is unsettled and the form must not
//! assert one.
//!
//! Username is wired to state; the rest of the fields are not. The seam
//! rejects a blank username with `StudioError::MissingUsername` rather than
//! letting `ConnectionBuilder` default it to `admin`, so this form has to
//! collect one and has to refuse to submit without it. Blocking the submit is
//! the point: a rejection the user cannot act on reads as a button that does
//! nothing.
//!
//! Host, port, auth method and password stay presentational because
//! `connect()` takes a name and credentials only, and no seam method persists
//! a new entry, so Save and Test have nothing to call. Wiring them needs a
//! seam addition, which is an ask-first change.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::services::backend::Backend;
use crate::state::connection::{ActiveConnection, ConnectError, apply_connect};
use crate::state::connections_registry::Credentials;
use crate::state::ui::ModalKind;

/// The credentials a submit should send, or `None` when the username is blank.
///
/// Extracted so the validation the form depends on is testable without a
/// renderer. Whitespace is not a username: the seam trims before its own
/// check, so a form that accepted `"   "` would just relay a rejection the
/// user has no way to read as "this field is empty".
fn submit_credentials(username: &str, password: &str) -> Option<Credentials> {
    let username = username.trim();
    if username.is_empty() {
        return None;
    }
    Some(Credentials {
        username: username.to_string(),
        password: (!password.is_empty()).then(|| password.to_string()),
    })
}

#[component]
pub fn NewConnectionForm() -> Element {
    let mut modal = use_context::<Signal<Option<ModalKind>>>();
    let mut active = use_context::<Signal<Option<ActiveConnection>>>();
    let mut connect_error = use_context::<Signal<ConnectError>>();
    let service = use_context::<Rc<dyn Backend>>();

    let mut name = use_signal(|| "local-nodedb-dev-2".to_string());
    let mut username = use_signal(String::new);
    let mut password = use_signal(String::new);
    // Raised by a submit attempt, not by typing: a form that scolds before the
    // user has entered anything is noise.
    let mut username_missing = use_signal(|| false);

    let submit = move |_| {
        let Some(creds) = submit_credentials(&username.peek(), &password.peek()) else {
            username_missing.set(true);
            return;
        };
        username_missing.set(false);
        let name = name.peek().clone();
        let service = service.clone();
        // Clear any previous failure so the surface reflects this attempt.
        connect_error.set(ConnectError(None));
        // spawn_forever so a Cancel mid-connect cannot cancel the attempt and
        // leave the app with neither a session nor an error.
        spawn_forever(async move {
            let result = service.connect(&name, &creds).await;
            let err = apply_connect(&mut active.write(), result);
            let connected = err.is_none();
            connect_error.set(ConnectError(err));
            // Close only on success: on failure the form stays open over the
            // error so the user can correct the field that caused it.
            if connected {
                modal.set(None);
            }
        });
    };

    rsx! {
        div { class: "modal-body",
            div { class: "form-field",
                label { "Name" }
                input {
                    value: "{name}",
                    oninput: move |e| name.set(e.value()),
                }
            }
            div { class: "form-row",
                div { class: "form-field", label { "Host" } input { value: "localhost" } }
                div { class: "form-field", label { "Port" } input { value: "2480" } }
            }
            div { class: "form-field",
                label { "Auth method" }
                select { option { "Username + password" } option { "Token" } option { "mTLS" } }
            }
            div { class: "form-row",
                div { class: "form-field",
                    label { "Username" }
                    input {
                        value: "{username}",
                        oninput: move |e| {
                            username.set(e.value());
                            username_missing.set(false);
                        },
                    }
                    if *username_missing.read() {
                        div { class: "field-error", "A username is required to connect." }
                    }
                }
                div { class: "form-field",
                    label { "Password" }
                    input {
                        r#type: "password",
                        value: "{password}",
                        oninput: move |e| password.set(e.value()),
                    }
                }
            }
            div { style: "display: flex; gap: 8px; align-items: center; padding-top: 6px;",
                span { class: "pill info", "i" }
                span { style: "font-size: 11px; color: var(--text-secondary);", "Credentials are stored in your OS keychain." }
            }
        }
        div { class: "modal-footer",
            button { class: "btn ghost", onclick: move |_| modal.set(None), "Cancel" }
            button { class: "btn", "Test" }
            button { class: "btn", "Save" }
            button { class: "btn primary", onclick: submit, "Save & connect" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_username_blocks_the_submit() {
        assert!(submit_credentials("", "hunter2").is_none());
    }

    /// Whitespace must not pass as a username. The seam trims before its own
    /// check, so relaying `"   "` would surface "a username is required" with
    /// a field that visibly contains something.
    #[test]
    fn whitespace_username_blocks_the_submit() {
        assert!(submit_credentials("   ", "hunter2").is_none());
    }

    #[test]
    fn username_is_trimmed_before_it_reaches_the_seam() {
        let creds = submit_credentials("  alice  ", "").expect("must submit");
        assert_eq!(creds.username, "alice");
    }

    #[test]
    fn empty_password_is_none_not_an_empty_string() {
        let creds = submit_credentials("alice", "").expect("must submit");
        assert!(
            creds.password.is_none(),
            "an empty field is an absent password, not a password of length zero"
        );
    }

    #[test]
    fn password_is_carried_when_present() {
        let creds = submit_credentials("alice", "hunter2").expect("must submit");
        assert_eq!(creds.password.as_deref(), Some("hunter2"));
    }
}
