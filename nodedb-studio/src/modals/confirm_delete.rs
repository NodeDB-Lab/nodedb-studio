//! Confirm-delete modal body, shared by every engine viewer.
//!
//! The destructive button stays disabled until the typed confirmation passes
//! `delete_confirmed`. SEAM-UNWIRED: Delete only closes the modal; no seam
//! method deletes a record yet.

use dioxus::prelude::*;

use crate::state::ui::ModalKind;

/// Whether `typed` confirms the delete. Surrounding whitespace is forgiven
/// (autocomplete, paste); case and spelling are not, so the friction before
/// an irreversible act stays.
fn delete_confirmed(typed: &str) -> bool {
    typed.trim() == "delete"
}

#[component]
pub fn ConfirmDelete() -> Element {
    let mut modal = use_context::<Signal<Option<ModalKind>>>();
    let mut typed = use_signal(String::new);
    let confirmed = delete_confirmed(&typed.read());
    rsx! {
        div { class: "modal-body",
            p { class: "confirm-lead", "Delete this record? This cannot be undone." }
            p { class: "confirm-detail",
                "The record will be removed from storage. If foreign keys reference it, the operation will fail unless cascade is configured."
            }
            div { class: "danger-zone", "Type " strong { "delete" } " to confirm." }
            input {
                class: "confirm-input",
                placeholder: "delete",
                value: "{typed}",
                oninput: move |e| typed.set(e.value()),
            }
        }
        div { class: "modal-footer",
            button { class: "btn ghost", onclick: move |_| modal.set(None), "Cancel" }
            button {
                class: "btn danger",
                disabled: !confirmed,
                onclick: move |_| modal.set(None),
                "Delete permanently"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_word_confirms() {
        assert!(delete_confirmed("delete"));
    }

    #[test]
    fn surrounding_whitespace_is_forgiven_but_case_is_not() {
        assert!(delete_confirmed(" delete "));
        assert!(delete_confirmed("delete\n"));
        assert!(!delete_confirmed("Delete"));
        assert!(!delete_confirmed("de lete"));
    }

    #[test]
    fn blank_and_partial_input_do_not_confirm() {
        assert!(!delete_confirmed(""));
        assert!(!delete_confirmed("del"));
        assert!(!delete_confirmed("deleted"));
    }
}
