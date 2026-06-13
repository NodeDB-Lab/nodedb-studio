//! New-connection modal body.
//!
//! Per CLAUDE.md §1 this is a single-engine client: there is NO Engine picker.
//! The Protocol field is also omitted — NodeDB's transport is undecided
//! (CLAUDE.md §2), so the form doesn't assert one. Static in the skeleton.

use dioxus::prelude::*;

use crate::state::ui::ModalKind;

#[component]
pub fn NewConnectionForm() -> Element {
    let mut modal = use_context::<Signal<Option<ModalKind>>>();
    rsx! {
        div { class: "modal-body",
            div { class: "form-field",
                label { "Name" }
                input { value: "local-nodedb-dev-2" }
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
                div { class: "form-field", label { "Username" } input { value: "root" } }
                div { class: "form-field", label { "Password" } input { r#type: "password", value: "••••••••" } }
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
            button { class: "btn primary", onclick: move |_| modal.set(None), "Save & connect" }
        }
    }
}
