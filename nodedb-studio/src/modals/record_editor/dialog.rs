//! The Insert / Edit record dialog.
//!
//! Review changes shows the diff and locks the editor, then Save writes.
//! The write names the revision the editor read, so a record changed in the
//! meantime is a conflict, shown here with the edit kept. The write runs
//! under `spawn_forever` and touches root state only.

use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::components::icon::{Ico, Icon};
use crate::components::modal::Modal;
use crate::modals::record_editor::body::{EditorTabs, FormEditor, JsonEditor};
use crate::modals::record_editor::side::{ChangesTable, SaveError, ValidationPanel};
use crate::services::backend::Backend;
use crate::state::explorer::{RecordView, RecordsEpoch};
use crate::state::record_editor::{EditorError, EditorMode, EditorTab, RecordEditor, Stage};
use crate::state::ui::ModalKind;

/// The root signals a save task writes.
#[derive(Clone, Copy)]
struct SaveEnv {
    editor: Signal<Option<RecordEditor>>,
    modal: Signal<Option<ModalKind>>,
    epoch: Signal<u32>,
    view: Signal<RecordView>,
}

async fn save_task(env: SaveEnv, backend: Rc<dyn Backend>, ed: RecordEditor) {
    let SaveEnv {
        mut editor,
        mut modal,
        mut epoch,
        mut view,
    } = env;
    let Ok(draft) = ed.draft() else {
        if let Some(e) = editor.write().as_mut() {
            e.busy = false;
        }
        return;
    };
    let result = match (&ed.mode, &ed.stored) {
        (EditorMode::Edit, Some(stored)) => {
            backend
                .update_record(&ed.collection, &stored.id, stored.revision, draft)
                .await
        }
        (EditorMode::Edit, None) | (EditorMode::Insert, _) => {
            backend
                .insert_record(&ed.collection, ed.insert_id(), draft)
                .await
        }
    };
    match result {
        Ok(saved) => {
            let next = *epoch.peek() + 1;
            epoch.set(next);
            view.write().record = Some(saved.id);
            modal.set(None);
        }
        Err(e) => {
            if let Some(open) = editor.write().as_mut() {
                open.busy = false;
                open.error = Some(EditorError::from(&e));
            }
        }
    }
}

/// Read the stored record again after a conflict and move the editor onto it.
async fn reload_task(env: SaveEnv, backend: Rc<dyn Backend>, collection: String, id: String) {
    let SaveEnv { mut editor, .. } = env;
    let read = backend.record(&collection, &id).await;
    if let Some(open) = editor.write().as_mut() {
        match read {
            Ok(stored) => open.rebase(stored),
            Err(e) => {
                open.busy = false;
                open.error = Some(EditorError::from(&e));
            }
        }
    }
}

#[component]
pub fn RecordEditorDialog() -> Element {
    let mut editor = use_context::<Signal<Option<RecordEditor>>>();
    let backend = use_context::<Rc<dyn Backend>>();
    let mut modal = use_context::<Signal<Option<ModalKind>>>();
    let epoch = use_context::<RecordsEpoch>();
    let view = use_context::<Signal<RecordView>>();
    let env = SaveEnv {
        editor,
        modal,
        epoch: epoch.0,
        view,
    };
    // A closed dialog keeps no edit.
    use_drop(move || editor.set(None));

    let Some(e) = editor.read().clone() else {
        return rsx! {};
    };
    let reviewing = e.stage == Stage::Reviewing;
    let insert = e.mode == EditorMode::Insert;
    let title = e.title();

    let save = {
        let backend = backend.clone();
        move |_| {
            let Some(ed) = editor.peek().clone() else {
                return;
            };
            if let Some(open) = editor.write().as_mut() {
                open.busy = true;
                open.error = None;
            }
            let backend = backend.clone();
            spawn_forever(async move { save_task(env, backend, ed).await });
        }
    };
    let reload = {
        let backend = backend.clone();
        move |_| {
            let Some(ed) = editor.peek().clone() else {
                return;
            };
            let Some(stored) = ed.stored.clone() else {
                return;
            };
            if let Some(open) = editor.write().as_mut() {
                open.busy = true;
            }
            let backend = backend.clone();
            spawn_forever(async move {
                reload_task(env, backend, ed.collection.clone(), stored.id).await;
            });
        }
    };

    rsx! {
        Modal {
            title,
            width: 1080,
            dismiss_on_scrim: false,
            class: "record-editor",
            div { class: "dialog-body editor-body",
                div { class: "editor-grid",
                    div { class: "editor-main",
                        div { class: "editor-meta",
                            div { class: "field",
                                label { r#for: "editor-collection", "Collection" }
                                select {
                                    id: "editor-collection",
                                    class: "select native",
                                    disabled: true,
                                    title: "The record belongs to the collection open in the Explorer",
                                    option { "{e.collection}" }
                                }
                            }
                            div { class: "field",
                                label { r#for: "editor-id", "Document ID" }
                                input {
                                    id: "editor-id",
                                    class: "input",
                                    value: "{e.id}",
                                    readonly: !insert || reviewing,
                                    placeholder: "Assigned by the server",
                                    oninput: move |ev| {
                                        if let Some(open) = editor.write().as_mut() {
                                            open.id = ev.value();
                                        }
                                    },
                                }
                            }
                        }
                        if let Some(err) = e.error.clone() {
                            SaveError { error: err, on_reload: reload }
                        }
                        EditorTabs { editor, tab: e.tab }
                        div { class: "editor-surface",
                            match e.tab {
                                EditorTab::Json => rsx! {
                                    JsonEditor {
                                        text: e.text.clone(),
                                        locked: e.busy || reviewing,
                                        on_input: move |t: String| {
                                            if let Some(open) = editor.write().as_mut() {
                                                open.text = t;
                                            }
                                        },
                                    }
                                },
                                EditorTab::Form => rsx! { FormEditor { editor } },
                            }
                        }
                    }
                    ValidationPanel { editor: e.clone() }
                }
                ChangesTable { editor: e.clone() }
            }
            footer { class: "dialog-footer",
                button {
                    class: "btn",
                    r#type: "button",
                    onclick: move |_| modal.set(None),
                    "Cancel"
                }
                if reviewing {
                    button {
                        class: "btn",
                        r#type: "button",
                        disabled: e.busy,
                        onclick: move |_| {
                            if let Some(open) = editor.write().as_mut() {
                                open.stage = Stage::Editing;
                            }
                        },
                        "Back to editing"
                    }
                    button {
                        class: "btn primary",
                        r#type: "button",
                        disabled: e.busy || matches!(e.error, Some(EditorError::Gone)),
                        onclick: save,
                        Ico { icon: Icon::Save, size: 16 }
                        if e.busy { "Saving…" } else if insert { "Insert record" } else { "Save changes" }
                    }
                } else {
                    button {
                        class: "btn",
                        r#type: "button",
                        disabled: true,
                        title: "Drafts are not designed yet",
                        "Save as draft"
                    }
                    button {
                        class: "btn primary",
                        r#type: "button",
                        disabled: !e.can_review(),
                        title: if e.can_review() { "" } else if insert { "Fix the validation errors first" } else { "Change a field, or fix the validation errors" },
                        onclick: move |_| {
                            if let Some(open) = editor.write().as_mut() {
                                open.stage = Stage::Reviewing;
                            }
                        },
                        Ico { icon: Icon::Play, size: 16 }
                        "Review changes"
                    }
                }
            }
        }
    }
}
