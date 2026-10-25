//! Dashboard document rules: opening, leaving, adding, removing. Plain
//! functions over `DashUi`, so each rule is tested without a renderer.

use crate::models::timeseries::{ExploreQuery, TimeSpan};
use crate::models::ts_dashboard::{Dashboard, MAX_PANEL_HEIGHT, MIN_PANEL_HEIGHT, PanelDef};
use crate::state::timeseries::{After, DashDialog, DashUi, Notice, PanelEditor, SaveState};

/// The key a panel's View entry and visibility use: the document and the
/// panel, because two documents may both have a `p1`.
pub fn panel_key(doc: &Dashboard, panel_id: &str) -> String {
    format!("{}:{panel_id}", doc.id.as_deref().unwrap_or("draft"))
}

/// Start from an empty, unsaved document.
pub fn start_draft(ui: &mut DashUi, range: TimeSpan) {
    let doc = Dashboard::untitled(range);
    ui.open_id = None;
    ui.open = Some(doc.clone());
    ui.baseline = Some(doc);
    ui.editor = None;
    ui.dialog = None;
    ui.menu = None;
    ui.removed = None;
    ui.save = SaveState::Idle;
}

/// Open a stored document. The page loads it and adopts it.
pub fn start_open(ui: &mut DashUi, id: &str) {
    ui.open_id = Some(id.to_string());
    ui.selected = Some(id.to_string());
    ui.open = None;
    ui.baseline = None;
    ui.editor = None;
    ui.dialog = None;
    ui.menu = None;
    ui.removed = None;
    ui.save = SaveState::Idle;
    ui.open_epoch += 1;
}

/// Ask to open `id`. Unsaved work in the open document asks first.
pub fn request_open(ui: &mut DashUi, id: &str) {
    if ui.open_id.as_deref() == Some(id) && ui.open.is_some() {
        return;
    }
    if ui.modified() {
        ui.dialog = Some(DashDialog::Unsaved(After::Open(id.to_string())));
    } else {
        start_open(ui, id);
    }
}

/// Ask for a new draft. Unsaved work in the open document asks first.
pub fn request_new(ui: &mut DashUi, range: TimeSpan) {
    if ui.modified() {
        ui.dialog = Some(DashDialog::Unsaved(After::New));
    } else {
        start_draft(ui, range);
    }
}

/// Carry out what a person asked for before the dialog stopped them.
pub fn proceed(ui: &mut DashUi, after: &After, range: TimeSpan) {
    match after {
        After::Open(id) => start_open(ui, id),
        After::New => start_draft(ui, range),
    }
}

/// Take a loaded document as the open one. A scene's edit, and a panel
/// another page asked to add, apply on top: both leave it modified.
pub fn adopt(ui: &mut DashUi, doc: Dashboard) {
    ui.baseline = Some(doc.clone());
    ui.open = Some(doc);
    ui.save = SaveState::Idle;
    if let Some(after) = ui.after_open.take() {
        if let (Some((panel, height)), Some(open)) = (after.resize, ui.open.as_mut()) {
            open.resize_panel(&panel, height);
        }
        if let Some(panel) = after.edit {
            open_editor(ui, &panel);
        }
    }
}

/// Put a panel another page asked for at the end of the open document.
/// Returns whether one was waiting.
pub fn take_pending_add(ui: &mut DashUi) -> bool {
    let (Some(panel), Some(open)) = (ui.pending_add.clone(), ui.open.as_mut()) else {
        return false;
    };
    ui.pending_add = None;
    let mut panel = panel;
    panel.id = open.next_panel_id();
    open.panels.push(panel);
    true
}

/// Edit panel `id` in the editor drawer.
pub fn open_editor(ui: &mut DashUi, id: &str) {
    if let Some(p) = ui.open.as_ref().and_then(|d| d.panel(id)) {
        ui.editor = Some(PanelEditor {
            target: Some(id.to_string()),
            draft: p.clone(),
        });
        ui.menu = None;
    }
}

/// Start a new panel from `query`, in the editor drawer.
pub fn new_panel(ui: &mut DashUi, query: ExploreQuery) {
    let Some(open) = ui.open.as_ref() else { return };
    ui.editor = Some(PanelEditor {
        target: None,
        draft: PanelDef {
            id: open.next_panel_id(),
            title: query.title(),
            query,
            kind: crate::models::timeseries::ChartKind::Line,
            height: 300,
        },
    });
    ui.menu = None;
}

/// Apply the editor's draft to the open document: replace the panel it
/// edits, or add a new one at the end. Closes the editor.
pub fn apply_editor(ui: &mut DashUi) {
    let (Some(editor), Some(open)) = (ui.editor.take(), ui.open.as_mut()) else {
        return;
    };
    let mut draft = editor.draft;
    draft.height = draft.height.clamp(MIN_PANEL_HEIGHT, MAX_PANEL_HEIGHT);
    match editor.target {
        Some(id) => {
            if let Some(slot) = open.panels.iter_mut().find(|p| p.id == id) {
                draft.id = id;
                *slot = draft;
            }
        }
        None => {
            draft.id = open.next_panel_id();
            open.panels.push(draft);
        }
    }
    ui.removed = None;
}

/// Remove a panel from the document, keeping it for Undo.
pub fn remove(ui: &mut DashUi, id: &str) {
    let Some(open) = ui.open.as_mut() else { return };
    if let Some((at, panel)) = open.remove_panel(id) {
        ui.notice = Some(Notice::ok(format!(
            "Removed {} from the dashboard. Undo before saving to keep it.",
            panel.title
        )));
        ui.removed = Some((at, panel));
    }
    ui.menu = None;
}

/// Put the last removed panel back where it was.
pub fn undo_remove(ui: &mut DashUi) {
    if let (Some((at, panel)), Some(open)) = (ui.removed.take(), ui.open.as_mut()) {
        open.insert_panel(at, panel);
        ui.notice = None;
    }
}

/// Discard the working copy: back to what was stored.
pub fn revert(ui: &mut DashUi) {
    ui.open = ui.baseline.clone();
    ui.editor = None;
    ui.removed = None;
    ui.save = SaveState::Idle;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::format::epoch;
    use crate::services::MockConnectionService;
    use crate::services::timeseries_data::TimeseriesData;
    use crate::state::timeseries::AfterOpen;

    fn range() -> TimeSpan {
        TimeSpan::new(epoch(2026, 6, 7, 0, 0, 0), epoch(2026, 6, 14, 0, 0, 0))
    }

    async fn loaded() -> DashUi {
        let svc = MockConnectionService::ready();
        let doc = svc.dashboard("dash-1").await.expect("doc");
        let mut ui = DashUi::default();
        start_open(&mut ui, "dash-1");
        adopt(&mut ui, doc);
        ui
    }

    #[tokio::test]
    async fn a_freshly_opened_document_is_not_modified() {
        let ui = loaded().await;
        assert!(!ui.modified());
        assert_eq!(ui.selected.as_deref(), Some("dash-1"));
    }

    #[tokio::test]
    async fn opening_over_unsaved_changes_asks_first() {
        let mut ui = loaded().await;
        ui.open.as_mut().expect("doc").resize_panel("p1", 400);
        assert!(ui.modified());
        request_open(&mut ui, "dash-2");
        assert_eq!(
            ui.dialog,
            Some(DashDialog::Unsaved(After::Open("dash-2".into())))
        );
        assert_eq!(ui.open_id.as_deref(), Some("dash-1"), "nothing moved yet");
        let after = After::Open("dash-2".into());
        proceed(&mut ui, &after, range());
        assert_eq!(ui.open_id.as_deref(), Some("dash-2"));
        assert!(ui.open.is_none() && ui.dialog.is_none());
    }

    #[tokio::test]
    async fn opening_the_open_document_changes_nothing() {
        let mut ui = loaded().await;
        request_open(&mut ui, "dash-1");
        assert!(ui.open.is_some());
        assert!(ui.dialog.is_none());
    }

    #[test]
    fn a_new_draft_is_unsaved_with_no_identity() {
        let mut ui = DashUi::default();
        request_new(&mut ui, range());
        let doc = ui.open.expect("draft");
        assert!(doc.id.is_none() && doc.panels.is_empty());
        assert_eq!(doc.range, range());
    }

    #[tokio::test]
    async fn the_editor_replaces_a_panel_in_place_or_adds_one() {
        let mut ui = loaded().await;
        open_editor(&mut ui, "p1");
        ui.editor.as_mut().expect("editor").draft.title = "Revenue".into();
        apply_editor(&mut ui);
        assert!(ui.editor.is_none());
        let doc = ui.open.clone().expect("doc");
        assert_eq!(doc.panels[0].title, "Revenue");
        assert_eq!(doc.panels[0].id, "p1");
        assert_eq!(doc.panels.len(), 2);
        assert!(ui.modified());

        let q = doc.panels[0].query.clone();
        new_panel(&mut ui, q);
        apply_editor(&mut ui);
        let doc = ui.open.expect("doc");
        assert_eq!(doc.panels.len(), 3);
        assert_eq!(doc.panels[2].id, "p3");
    }

    #[tokio::test]
    async fn removing_a_panel_can_be_undone_until_saved() {
        let mut ui = loaded().await;
        remove(&mut ui, "p1");
        assert_eq!(ui.open.as_ref().map(|d| d.panels.len()), Some(1));
        assert!(ui.modified());
        undo_remove(&mut ui);
        assert!(!ui.modified(), "undo restores the stored document exactly");
    }

    #[tokio::test]
    async fn a_scene_edit_applies_after_the_document_loads() {
        let svc = MockConnectionService::ready();
        let doc = svc.dashboard("dash-1").await.expect("doc");
        let mut ui = DashUi {
            after_open: Some(AfterOpen {
                resize: Some(("p2".into(), 262)),
                edit: Some("p1".into()),
            }),
            ..DashUi::default()
        };
        start_open(&mut ui, "dash-1");
        adopt(&mut ui, doc);
        assert!(ui.modified());
        assert_eq!(
            ui.editor.as_ref().and_then(|e| e.target.as_deref()),
            Some("p1")
        );
    }

    #[tokio::test]
    async fn a_panel_added_from_explore_lands_at_the_end_with_a_fresh_id() {
        let mut ui = loaded().await;
        let mut panel = ui.open.as_ref().expect("doc").panels[0].clone();
        panel.id = String::new();
        ui.pending_add = Some(panel);
        assert!(take_pending_add(&mut ui));
        let doc = ui.open.as_ref().expect("doc");
        assert_eq!(doc.panels.last().map(|p| p.id.as_str()), Some("p3"));
        assert!(!take_pending_add(&mut ui));
        assert!(ui.modified());
    }

    #[tokio::test]
    async fn revert_returns_to_the_stored_document() {
        let mut ui = loaded().await;
        remove(&mut ui, "p2");
        revert(&mut ui);
        assert!(!ui.modified());
        assert_eq!(ui.open.as_ref().map(|d| d.panels.len()), Some(2));
    }

    #[test]
    fn two_documents_never_share_a_panel_key() {
        let a = Dashboard {
            id: Some("dash-1".into()),
            ..Dashboard::untitled(range())
        };
        let b = Dashboard {
            id: Some("dash-2".into()),
            ..Dashboard::untitled(range())
        };
        assert_ne!(panel_key(&a, "p1"), panel_key(&b, "p1"));
        assert_eq!(panel_key(&Dashboard::untitled(range()), "p1"), "draft:p1");
    }
}
