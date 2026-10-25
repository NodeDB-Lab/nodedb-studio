//! The validation panel, the changes table, and the save error.

use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::models::record_check::ChangeKind;
use crate::state::record_editor::{EditorError, EditorMode, RecordEditor};

#[component]
pub fn ValidationPanel(editor: RecordEditor) -> Element {
    let changes = editor.changes();
    let problems = editor.problems();
    let edit = editor.mode == EditorMode::Edit;
    let id_line = if edit {
        "Immutable"
    } else {
        "Typed or assigned by the server"
    };
    let revision_line = if edit {
        "Enabled"
    } else {
        "Not applicable to a new record"
    };
    rsx! {
        aside { class: "editor-side", "aria-label": "Validation",
            h3 { class: "side-title", "Validation" }
            div { class: "kv kv-compact",
                div { class: "kv-row", span { "Schema" } span { "{editor.profile.schema_label()}" } }
                div { class: "kv-row", span { "ID" } span { "{id_line}" } }
                div { class: "kv-row", span { "Changed fields" } span { "{changes.len()}" } }
                div { class: "kv-row", span { "Revision check" } span { "{revision_line}" } }
            }
            h4 { class: "inspector-section", "Changed fields ({changes.len()})" }
            if changes.is_empty() {
                p { class: "meta", if edit { "No field differs from revision {editor.stored.as_ref().map_or(0, |r| r.revision)}." } else { "The record has no fields yet." } }
            }
            for c in changes.iter() {
                div { key: "{c.field}", class: "change-line",
                    span { class: "change-field", "{c.field}" }
                    span { class: "badge blue", "{c.kind.label()}" }
                    span { class: "change-values",
                        if let Some(b) = &c.before { span { class: "was", "{unquote(b)}" } }
                        if c.before.is_some() && c.after.is_some() { span { class: "arrow", "aria-hidden": "true", "→" } }
                        if let Some(a) = &c.after { span { "{unquote(a)}" } }
                    }
                }
            }
            h4 { class: "inspector-section", "Validation" }
            if problems.is_empty() {
                div { class: "validation ok",
                    Ico { icon: Icon::CircleCheck, size: 20 }
                    div {
                        strong { if editor.profile.enforces_schema() { "Record is valid" } else { "Document is valid" } }
                        p { "No schema violations detected." }
                    }
                }
            } else {
                for p in problems.iter() {
                    div { key: "{p.field}{p.message}", class: "validation bad", role: "alert",
                        Ico { icon: Icon::CircleAlert, size: 20 }
                        div {
                            if p.field.is_empty() {
                                strong { "{p.message}" }
                            } else {
                                strong { "{p.field}" }
                                p { "{p.message}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// A JSON string value without its quotes, for a short before/after.
fn unquote(text: &str) -> String {
    text.trim_matches('"').to_string()
}

#[component]
pub fn ChangesTable(editor: RecordEditor) -> Element {
    let changes = editor.changes();
    let stored = editor.stored.as_ref().map_or_else(
        || "Stored".to_string(),
        |r| format!("Stored revision {}", r.revision),
    );
    rsx! {
        section { class: "editor-changes", "aria-label": "Changes",
            h3 { class: "side-title", "Changes ({changes.len()})" }
            table { class: "dt dt-compact",
                thead { tr { th { "Field" } th { "{stored}" } th { "Proposed" } } }
                tbody {
                    for c in changes.iter() {
                        tr { key: "{c.field}",
                            td { "{c.field}" }
                            td { class: if c.kind == ChangeKind::Added { "diff-empty" } else { "diff-del mono" },
                                if let Some(b) = &c.before { "{b}" }
                            }
                            td { class: if c.kind == ChangeKind::Removed { "diff-empty" } else { "diff-add mono" },
                                if let Some(a) = &c.after { "{a}" }
                            }
                        }
                    }
                }
            }
            if changes.is_empty() {
                p { class: "meta pad", "No changes yet." }
            }
        }
    }
}

/// The save error, with the way out each kind has.
#[component]
pub fn SaveError(error: EditorError, on_reload: EventHandler<()>) -> Element {
    match error {
        EditorError::Conflict { expected, stored } => rsx! {
            div { class: "callout warn", role: "alert",
                Ico { icon: Icon::TriangleAlert, size: 16 }
                div { class: "callout-body",
                    strong { "Revision conflict" }
                    p { "You edited revision {expected}, and the server now holds revision {stored}. Your edit is kept, and nothing was saved." }
                    button { class: "btn sm", onclick: move |_| on_reload.call(()), "Load revision {stored}" }
                }
            }
        },
        EditorError::Gone => rsx! {
            div { class: "callout err", role: "alert",
                Ico { icon: Icon::CircleAlert, size: 16 }
                div { class: "callout-body",
                    strong { "The record no longer exists" }
                    p { "Someone removed it after you opened it. Your edit is kept in this dialog." }
                }
            }
        },
        EditorError::Denied(text) | EditorError::Rejected(text) | EditorError::Failed(text) => {
            rsx! {
                div { class: "callout err", role: "alert",
                    Ico { icon: Icon::CircleAlert, size: 16 }
                    div { class: "callout-body",
                        strong { "Save did not complete" }
                        p { "{text}" }
                    }
                }
            }
        }
    }
}
