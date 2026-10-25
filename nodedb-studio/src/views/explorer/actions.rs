//! Opening the record dialogs from the Explorer.
//!
//! The dialogs read their state from root signals, so the openers only fill
//! those signals and show the modal.

use dioxus::prelude::*;

use crate::models::collection::Collection;
use crate::models::records::Record;
use crate::models::schema::CollectionSchema;
use crate::state::record_editor::{DeleteRequest, DeleteTarget, RecordEditor};
use crate::state::ui::ModalKind;

#[derive(Clone, Copy)]
pub struct Launcher {
    editor: Signal<Option<RecordEditor>>,
    delete: Signal<Option<DeleteRequest>>,
    modal: Signal<Option<ModalKind>>,
}

/// The openers. Call in a component body, use in its handlers.
pub fn use_launcher() -> Launcher {
    Launcher {
        editor: use_context(),
        delete: use_context(),
        modal: use_context(),
    }
}

impl Launcher {
    pub fn edit(mut self, collection: &Collection, schema: &CollectionSchema, record: &Record) {
        self.editor.set(Some(RecordEditor::edit(
            &collection.name,
            collection.profile,
            schema.clone(),
            record.clone(),
        )));
        self.modal.set(Some(ModalKind::RecordEditor));
    }

    pub fn insert(mut self, collection: &Collection, schema: &CollectionSchema) {
        self.editor.set(Some(RecordEditor::insert(
            &collection.name,
            collection.profile,
            schema.clone(),
        )));
        self.modal.set(Some(ModalKind::RecordEditor));
    }

    pub fn delete_record(mut self, collection: &str, record: &Record) {
        self.delete
            .set(Some(DeleteRequest::new(DeleteTarget::Record {
                collection: collection.to_string(),
                id: record.id.clone(),
                revision: record.revision,
            })));
        self.modal.set(Some(ModalKind::ConfirmDelete));
    }

    pub fn delete_edge(mut self, graph: &str, id: &str) {
        self.delete.set(Some(DeleteRequest::new(DeleteTarget::Edge {
            graph: graph.to_string(),
            id: id.to_string(),
        })));
        self.modal.set(Some(ModalKind::ConfirmDelete));
    }
}
