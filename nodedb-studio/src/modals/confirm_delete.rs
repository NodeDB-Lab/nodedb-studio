//! The delete confirmation for a record or an edge.
//!
//! The destructive button stays disabled until the word is typed. A record
//! delete names the revision it read: a record changed since is a conflict,
//! shown here, and nothing is removed.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::modal::Modal;
use crate::services::backend::Backend;
use crate::state::explorer::RecordsEpoch;
use crate::state::record_editor::{DeleteRequest, DeleteTarget, EditorError};
use crate::state::ui::ModalKind;

fn error_text(error: &EditorError) -> String {
    match error {
        EditorError::Conflict { expected, stored } => format!(
            "The record changed after you opened it: you read revision {expected}, the server holds revision {stored}. Nothing was removed. Close this dialog and review the record."
        ),
        EditorError::Gone => "The record no longer exists.".to_string(),
        EditorError::Denied(t) | EditorError::Rejected(t) | EditorError::Failed(t) => t.clone(),
    }
}

#[component]
pub fn ConfirmDelete() -> Element {
    let mut request = use_context::<Signal<Option<DeleteRequest>>>();
    let mut modal = use_context::<Signal<Option<ModalKind>>>();
    let backend = use_context::<Rc<dyn Backend>>();
    let epoch = use_context::<RecordsEpoch>();
    use_drop(move || request.set(None));

    let Some(r) = request.read().clone() else {
        return rsx! {};
    };
    let label = r.target.label();
    let remove = {
        let backend = backend.clone();
        move |_| {
            let Some(current) = request.peek().clone() else {
                return;
            };
            if !current.confirmed() || current.busy {
                return;
            }
            if let Some(open) = request.write().as_mut() {
                open.busy = true;
                open.error = None;
            }
            let backend = backend.clone();
            let (mut epoch, mut request, mut modal) = (epoch.0, request, modal);
            spawn_forever(async move {
                let result = match &current.target {
                    DeleteTarget::Record {
                        collection,
                        id,
                        revision,
                    } => backend.delete_record(collection, id, *revision).await,
                    DeleteTarget::Edge { graph, id } => backend.delete_edge(graph, id).await,
                };
                match result {
                    Ok(()) => {
                        let next = *epoch.peek() + 1;
                        epoch.set(next);
                        modal.set(None);
                    }
                    Err(e) => {
                        if let Some(open) = request.write().as_mut() {
                            open.busy = false;
                            open.error = Some(EditorError::from(&e));
                        }
                    }
                }
            });
        }
    };

    rsx! {
        Modal { title: "Delete {label}", width: 460, dismiss_on_scrim: false,
            div { class: "dialog-body",
                p { class: "confirm-lead", "Delete {label}? This cannot be undone." }
                p { class: "confirm-detail",
                    match &r.target {
                        DeleteTarget::Record { .. } => "The record is removed from storage. A reference to it from another collection is not checked and can dangle.",
                        DeleteTarget::Edge { .. } => "The edge is removed from the graph. The records it joined stay.",
                    }
                }
                if let Some(e) = &r.error {
                    div { class: "callout err", role: "alert", "{error_text(e)}" }
                }
                div { class: "danger-zone", "Type " strong { "delete" } " to confirm." }
                input {
                    class: "input",
                    placeholder: "delete",
                    "aria-label": "Type delete to confirm",
                    autofocus: true,
                    value: "{r.typed}",
                    oninput: move |e| {
                        if let Some(open) = request.write().as_mut() {
                            open.typed = e.value();
                        }
                    },
                }
            }
            footer { class: "dialog-footer",
                button { class: "btn", onclick: move |_| modal.set(None), "Cancel" }
                button {
                    class: "btn danger",
                    disabled: !r.confirmed() || r.busy,
                    onclick: remove,
                    if r.busy { "Deleting…" } else { "Delete permanently" }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_conflict_says_nothing_was_removed() {
        let text = error_text(&EditorError::Conflict {
            expected: 18,
            stored: 19,
        });
        assert!(
            text.contains("revision 18") && text.contains("revision 19"),
            "{text}"
        );
        assert!(text.contains("Nothing was removed"), "{text}");
    }
}
